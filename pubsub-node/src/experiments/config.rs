//! Sweep-description parsing and validation: the loader layer between the
//! front-end binary's file/argument handling and the experiments API's
//! already-parsed values.
//!
//! Everything result-affecting lives in the TOML sweep description and is
//! validated here, before any run executes; invocation flags (output
//! directory, worker count, detail) stay on the binary and never reach the
//! manifest.
// 016-FR-028 (axes), 016-FR-031; contracts/sweep-config.md.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::peer::PeerId;
use crate::strategies::acceptance::AcceptanceStrategyKind;
use crate::strategies::connection::ConnectionStrategyKind;
use crate::topic::TopicId;

use super::graph::DisseminationModel;
use super::population::{AcceptanceSpec, ConnectionSpec, FanoutSpec, StrategySpec};

/// A rejected sweep description. Messages are operator-facing.
#[derive(Debug, thiserror::Error)]
pub enum SweepConfigError {
    /// The TOML failed to parse (malformed syntax or unknown fields).
    #[error("invalid sweep description: {0}")]
    Parse(#[from] toml::de::Error),
    /// The dissemination model is not one this build knows.
    #[error("{0}")]
    UnknownModel(#[from] super::graph::UnknownDisseminationModel),
    /// A strategy field named no known kind.
    #[error("unknown {seam} strategy '{kind}' (expected one of: {expected})")]
    UnknownStrategy {
        /// Which seam the field configures.
        seam: &'static str,
        /// The offending kind string.
        kind: String,
        /// The accepted names.
        expected: &'static str,
    },
    /// A strategy rejected its parameters.
    #[error("{0}")]
    Strategy(#[from] crate::strategies::config::StrategyConfigError),
    /// The topic string is not a valid topic id.
    #[error("invalid topic: {0}")]
    Topic(#[from] crate::topic::TopicIdError),
    /// Two spellings of the same knob were both set.
    #[error("set either {a} or {b}, not both")]
    ConflictingFields {
        /// One spelling.
        a: &'static str,
        /// The other spelling.
        b: &'static str,
    },
    /// A fraction fell outside [0, 1].
    #[error("{field} must be between 0 and 1 (got {value})")]
    FractionOutOfRange {
        /// The offending field.
        field: &'static str,
        /// The rejected value.
        value: f64,
    },
    /// A count that must be positive was zero.
    #[error("{field} must be at least 1")]
    ZeroCount {
        /// The offending field.
        field: &'static str,
    },
    /// An axis named no sweepable parameter.
    #[error("unknown axis parameter '{name}' (expected one of: size, adversarial, adversarial_fraction, churn, churn_count, target_degree, publishes_per_run)")]
    UnknownAxisParameter {
        /// The offending parameter name.
        name: String,
    },
    /// The same parameter appeared on two axes.
    #[error("axis parameter '{parameter}' is declared twice")]
    DuplicateAxis {
        /// The duplicated parameter name.
        parameter: &'static str,
    },
    /// An axis carried no values.
    #[error("axis '{parameter}' needs at least one value")]
    EmptyAxis {
        /// The empty axis's parameter name.
        parameter: &'static str,
    },
    /// An axis value had the wrong shape for its parameter.
    #[error("axis '{parameter}' takes {expected} values (got {value})")]
    AxisValueType {
        /// The axis's parameter name.
        parameter: &'static str,
        /// What the parameter accepts.
        expected: &'static str,
        /// The rejected value.
        value: String,
    },
    /// The population leaves no publisher/receiver pair.
    #[error("the population must keep at least two honest participants up (a publisher and a receiver): size {size}, adversarial {adversarial}, churn {churn} leaves {remaining}")]
    TooFewUpHonest {
        /// Configured population size.
        size: usize,
        /// Resolved adversarial count.
        adversarial: usize,
        /// Resolved churn count.
        churn: usize,
        /// Up-honest nodes remaining after the draw.
        remaining: usize,
    },
}

/// One class's strategy configuration, exactly as validated — kept in this
/// raw-shaped form so the manifest can embed it verbatim.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StrategyTable {
    /// The dial kind: a protocol kind or `uniform-sampler`.
    pub connection: String,
    /// Target degree, where the dial or acceptance kind requires one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_degree: Option<usize>,
    /// Optional pinned bucket count for the hash-gated kinds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bucket_count: Option<usize>,
    /// The acceptance kind (protocol kinds only).
    pub acceptance: String,
    /// Accept-cap buffer for the bounded acceptance kinds.
    #[serde(default = "default_cap_buffer")]
    pub cap_buffer: usize,
    /// The fan-out kind: `forward-to-all` or `silent-relay`.
    pub fanout: String,
}

fn default_cap_buffer() -> usize {
    3
}

impl StrategyTable {
    /// Resolve the table into a buildable [`StrategySpec`], validating every
    /// kind name and the parameters each kind requires (probed with a
    /// placeholder identity so rejection happens before any run executes).
    pub fn to_spec(&self) -> Result<StrategySpec, SweepConfigError> {
        let connection = match self.connection.to_ascii_lowercase().as_str() {
            "uniform-sampler" => {
                let target_degree = self.target_degree.ok_or(
                    crate::strategies::config::StrategyConfigError::MissingParameter {
                        strategy: "uniform-sampler",
                        parameter: "a target degree (target_degree)",
                    },
                )?;
                if target_degree == 0 {
                    return Err(SweepConfigError::ZeroCount {
                        field: "target_degree",
                    });
                }
                ConnectionSpec::UniformSampler { target_degree }
            }
            other => {
                let kind = ConnectionStrategyKind::from_str(other).map_err(|_| {
                    SweepConfigError::UnknownStrategy {
                        seam: "connection",
                        kind: self.connection.clone(),
                        expected: "connect-to-all, hash-gated, uniform-sampler",
                    }
                })?;
                ConnectionSpec::Protocol {
                    kind,
                    target_degree: self.target_degree,
                    bucket_count: self.bucket_count,
                }
            }
        };
        let acceptance_kind = AcceptanceStrategyKind::from_str(&self.acceptance).map_err(|_| {
            SweepConfigError::UnknownStrategy {
                seam: "acceptance",
                kind: self.acceptance.clone(),
                expected: "accept-from-all, bounded, hash-gated, hash-gated-bounded",
            }
        })?;
        let acceptance = AcceptanceSpec::Protocol {
            kind: acceptance_kind,
            target_degree: self.target_degree,
            bucket_count: self.bucket_count,
            cap_buffer: self.cap_buffer,
        };
        let fanout = match self.fanout.to_ascii_lowercase().as_str() {
            "forward-to-all" => FanoutSpec::ForwardToAll,
            "silent-relay" => FanoutSpec::SilentRelay,
            _ => {
                return Err(SweepConfigError::UnknownStrategy {
                    seam: "fan-out",
                    kind: self.fanout.clone(),
                    expected: "forward-to-all, silent-relay",
                })
            }
        };
        let spec = StrategySpec {
            connection,
            acceptance,
            fanout,
        };
        spec.probe(&PeerId::from_str("probe").expect("static probe id"))?;
        Ok(spec)
    }
}

/// A population knob given as an absolute count or a fraction of its basis
/// (adversarial: fraction of N; churn: fraction of the honest population).
/// Kept unresolved so an axis that changes the basis (e.g. sweeping `size`)
/// re-resolves per grid point.
#[derive(Clone, Copy, Debug)]
pub enum CountSpec {
    /// An absolute count.
    Count(usize),
    /// A fraction of the knob's basis, rounded to the nearest count.
    Fraction(f64),
}

impl CountSpec {
    fn resolve(self, field: &'static str, basis: usize) -> Result<usize, SweepConfigError> {
        match self {
            Self::Count(count) => Ok(count),
            Self::Fraction(fraction) => resolve_fraction(field, fraction, basis),
        }
    }
}

/// A sweepable parameter: one axis of the sweep's grid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisParameter {
    /// Population size N.
    Size,
    /// Adversarial count.
    Adversarial,
    /// Adversarial fraction of N.
    AdversarialFraction,
    /// Churn proportion of the honest population.
    Churn,
    /// Churn count.
    ChurnCount,
    /// Target degree, applied to both classes' strategy tables.
    TargetDegree,
    /// Publish phases per run.
    PublishesPerRun,
}

impl AxisParameter {
    /// The configuration spelling.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Size => "size",
            Self::Adversarial => "adversarial",
            Self::AdversarialFraction => "adversarial_fraction",
            Self::Churn => "churn",
            Self::ChurnCount => "churn_count",
            Self::TargetDegree => "target_degree",
            Self::PublishesPerRun => "publishes_per_run",
        }
    }

    fn parse(name: &str) -> Result<Self, SweepConfigError> {
        match name {
            "size" => Ok(Self::Size),
            "adversarial" => Ok(Self::Adversarial),
            "adversarial_fraction" => Ok(Self::AdversarialFraction),
            "churn" => Ok(Self::Churn),
            "churn_count" => Ok(Self::ChurnCount),
            "target_degree" => Ok(Self::TargetDegree),
            "publishes_per_run" => Ok(Self::PublishesPerRun),
            _ => Err(SweepConfigError::UnknownAxisParameter {
                name: name.to_string(),
            }),
        }
    }
}

/// One raw axis value: TOML integers and floats both appear in practice
/// (`[0, 0.05]`); each parameter coerces what it accepts.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(untagged)]
pub enum AxisValue {
    /// A TOML integer.
    Integer(u64),
    /// A TOML float.
    Float(f64),
}

impl AxisValue {
    fn as_count(self, parameter: &'static str) -> Result<usize, SweepConfigError> {
        match self {
            Self::Integer(value) => Ok(usize::try_from(value).expect("counts fit usize")),
            Self::Float(value) => Err(SweepConfigError::AxisValueType {
                parameter,
                expected: "integer",
                value: value.to_string(),
            }),
        }
    }

    #[allow(clippy::cast_precision_loss)] // axis counts ≪ 2^52
    fn as_fraction(self) -> f64 {
        match self {
            Self::Integer(value) => value as f64,
            Self::Float(value) => value,
        }
    }
}

/// One validated axis: a sweepable parameter and its value list.
#[derive(Clone, Debug)]
pub struct Axis {
    /// The swept parameter.
    pub parameter: AxisParameter,
    /// The values, in declaration order.
    pub values: Vec<AxisValue>,
}

/// One fully-resolved grid point of the sweep: the per-experiment scalars
/// after axis overrides and count resolution.
#[derive(Clone, Debug)]
pub struct ResolvedExperiment {
    /// Population size N.
    pub size: usize,
    /// Resolved adversarial count.
    pub adversarial: usize,
    /// Resolved churn count.
    pub churn_count: usize,
    /// Honest-class strategy configuration (axis overrides applied).
    pub honest_strategies: StrategyTable,
    /// Adversarial-class strategy configuration (axis overrides applied).
    pub adversarial_strategies: StrategyTable,
    /// Publish phases per run.
    pub publishes_per_run: u64,
}

/// A parsed, validated sweep description: every result-affecting input.
/// Population knobs stay as count-or-fraction specs so each grid point
/// resolves against its own basis; [`SweepDescription::resolved_experiments`]
/// expands the axes' cross-product in declaration order.
#[derive(Clone, Debug)]
pub struct SweepDescription {
    /// The dissemination model.
    pub model: DisseminationModel,
    /// The sweep's master seed.
    pub master_seed: u64,
    /// Population size N (base value; a `size` axis overrides it).
    pub size: usize,
    /// Adversarial knob (base value).
    pub adversarial: CountSpec,
    /// Churn knob (base value).
    pub churn: CountSpec,
    /// The single topic.
    pub topic: TopicId,
    /// Honest-class strategy configuration (base).
    pub honest_strategies: StrategyTable,
    /// Adversarial-class strategy configuration (base).
    pub adversarial_strategies: StrategyTable,
    /// Runs per experiment R.
    pub runs_per_experiment: u64,
    /// Publish phases per run (base value; default 1).
    pub publishes_per_run: u64,
    /// The sweep axes, in declaration order (empty ⇒ one experiment).
    pub axes: Vec<Axis>,
}

impl SweepDescription {
    /// Expand the axes' cross-product — first-declared axis varying slowest
    /// — into fully-resolved, validated experiments. Called at parse time so
    /// every grid point is rejected or accepted before anything runs.
    pub fn resolved_experiments(&self) -> Result<Vec<ResolvedExperiment>, SweepConfigError> {
        let mut combinations: Vec<Vec<(AxisParameter, AxisValue)>> = vec![Vec::new()];
        for axis in &self.axes {
            let mut expanded = Vec::with_capacity(combinations.len() * axis.values.len());
            for combination in &combinations {
                for &value in &axis.values {
                    let mut next = combination.clone();
                    next.push((axis.parameter, value));
                    expanded.push(next);
                }
            }
            combinations = expanded;
        }

        combinations
            .into_iter()
            .map(|combination| self.resolve_combination(&combination))
            .collect()
    }

    /// Apply one grid point's overrides and resolve/validate it.
    fn resolve_combination(
        &self,
        overrides: &[(AxisParameter, AxisValue)],
    ) -> Result<ResolvedExperiment, SweepConfigError> {
        let mut size = self.size;
        let mut adversarial = self.adversarial;
        let mut churn = self.churn;
        let mut publishes_per_run = self.publishes_per_run;
        let mut honest_strategies = self.honest_strategies.clone();
        let mut adversarial_strategies = self.adversarial_strategies.clone();

        for &(parameter, value) in overrides {
            match parameter {
                AxisParameter::Size => size = value.as_count(parameter.name())?,
                AxisParameter::Adversarial => {
                    adversarial = CountSpec::Count(value.as_count(parameter.name())?);
                }
                AxisParameter::AdversarialFraction => {
                    adversarial = CountSpec::Fraction(value.as_fraction());
                }
                AxisParameter::Churn => churn = CountSpec::Fraction(value.as_fraction()),
                AxisParameter::ChurnCount => {
                    churn = CountSpec::Count(value.as_count(parameter.name())?);
                }
                AxisParameter::TargetDegree => {
                    let degree = value.as_count(parameter.name())?;
                    honest_strategies.target_degree = Some(degree);
                    adversarial_strategies.target_degree = Some(degree);
                }
                AxisParameter::PublishesPerRun => {
                    let publishes = value.as_count(parameter.name())?;
                    if publishes == 0 {
                        return Err(SweepConfigError::ZeroCount {
                            field: "publishes_per_run",
                        });
                    }
                    publishes_per_run = publishes as u64;
                }
            }
        }

        if size == 0 {
            return Err(SweepConfigError::ZeroCount {
                field: "population.size",
            });
        }
        let adversarial = adversarial.resolve("population.adversarial_fraction", size)?;
        let honest = size.saturating_sub(adversarial);
        let churn_count = churn.resolve("population.churn", honest)?;
        let remaining = honest.saturating_sub(churn_count);
        if adversarial > size || remaining < 2 {
            return Err(SweepConfigError::TooFewUpHonest {
                size,
                adversarial,
                churn: churn_count,
                remaining,
            });
        }

        // Probe both tables per grid point (a target-degree override can
        // change what a kind requires).
        honest_strategies.to_spec()?;
        adversarial_strategies.to_spec()?;

        Ok(ResolvedExperiment {
            size,
            adversarial,
            churn_count,
            honest_strategies,
            adversarial_strategies,
            publishes_per_run,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSweepConfig {
    model: String,
    master_seed: u64,
    population: RawPopulation,
    strategies: RawStrategies,
    execution: RawExecution,
    #[serde(default)]
    axes: Vec<RawAxis>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAxis {
    parameter: String,
    values: Vec<AxisValue>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPopulation {
    size: usize,
    #[serde(default)]
    adversarial: Option<usize>,
    #[serde(default)]
    adversarial_fraction: Option<f64>,
    #[serde(default)]
    churn: Option<f64>,
    #[serde(default)]
    churn_count: Option<usize>,
    topic: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawStrategies {
    honest: StrategyTable,
    adversarial: StrategyTable,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExecution {
    runs_per_experiment: u64,
    #[serde(default)]
    publishes_per_run: Option<u64>,
}

/// Round a fraction of `whole` to the nearest count.
fn resolve_fraction(
    field: &'static str,
    fraction: f64,
    whole: usize,
) -> Result<usize, SweepConfigError> {
    if !(0.0..=1.0).contains(&fraction) {
        return Err(SweepConfigError::FractionOutOfRange {
            field,
            value: fraction,
        });
    }
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    // populations ≪ 2^52; the product is within [0, whole]
    Ok((fraction * whole as f64).round() as usize)
}

/// Parse and validate a sweep description from its TOML text.
///
/// Rejections happen here, before any run executes: unknown model or
/// strategy kind, conflicting count/fraction spellings, out-of-range
/// fractions, zero sizes or run counts, and populations that leave no
/// up-honest publisher/receiver pair after the churn draw.
pub fn parse_sweep_description(text: &str) -> Result<SweepDescription, SweepConfigError> {
    let raw: RawSweepConfig = toml::from_str(text)?;
    let model = DisseminationModel::from_str(&raw.model)?;
    let topic = TopicId::from_str(&raw.population.topic)?;

    if raw.population.size == 0 {
        return Err(SweepConfigError::ZeroCount {
            field: "population.size",
        });
    }
    if raw.execution.runs_per_experiment == 0 {
        return Err(SweepConfigError::ZeroCount {
            field: "execution.runs_per_experiment",
        });
    }
    let publishes_per_run = raw.execution.publishes_per_run.unwrap_or(1);
    if publishes_per_run == 0 {
        return Err(SweepConfigError::ZeroCount {
            field: "execution.publishes_per_run",
        });
    }

    let adversarial = match (
        raw.population.adversarial,
        raw.population.adversarial_fraction,
    ) {
        (Some(_), Some(_)) => {
            return Err(SweepConfigError::ConflictingFields {
                a: "population.adversarial",
                b: "population.adversarial_fraction",
            })
        }
        (Some(count), None) => CountSpec::Count(count),
        (None, Some(fraction)) => CountSpec::Fraction(fraction),
        (None, None) => CountSpec::Count(0),
    };

    // The churn proportion applies to the honest population — only honest
    // nodes churn.
    let churn = match (raw.population.churn_count, raw.population.churn) {
        (Some(_), Some(_)) => {
            return Err(SweepConfigError::ConflictingFields {
                a: "population.churn_count",
                b: "population.churn",
            })
        }
        (Some(count), None) => CountSpec::Count(count),
        (None, Some(fraction)) => CountSpec::Fraction(fraction),
        (None, None) => CountSpec::Count(0),
    };

    let mut axes = Vec::with_capacity(raw.axes.len());
    for axis in raw.axes {
        let parameter = AxisParameter::parse(&axis.parameter)?;
        if axes
            .iter()
            .any(|existing: &Axis| existing.parameter == parameter)
        {
            return Err(SweepConfigError::DuplicateAxis {
                parameter: parameter.name(),
            });
        }
        if axis.values.is_empty() {
            return Err(SweepConfigError::EmptyAxis {
                parameter: parameter.name(),
            });
        }
        axes.push(Axis {
            parameter,
            values: axis.values,
        });
    }

    let description = SweepDescription {
        model,
        master_seed: raw.master_seed,
        size: raw.population.size,
        adversarial,
        churn,
        topic,
        honest_strategies: raw.strategies.honest,
        adversarial_strategies: raw.strategies.adversarial,
        runs_per_experiment: raw.execution.runs_per_experiment,
        publishes_per_run,
        axes,
    };

    // Expand and validate every grid point (strategy probes included) so a
    // bad combination rejects the sweep before anything runs.
    description.resolved_experiments()?;

    Ok(description)
}

#[cfg(test)]
mod tests {
    use super::{parse_sweep_description, SweepConfigError};

    fn base_toml() -> String {
        r#"
            model = "m2"
            master_seed = 42

            [population]
            size = 20
            adversarial = 2
            churn = 0.1
            topic = "t0"

            [strategies.honest]
            connection = "uniform-sampler"
            target_degree = 4
            acceptance = "accept-from-all"
            fanout = "forward-to-all"

            [strategies.adversarial]
            connection = "uniform-sampler"
            target_degree = 4
            acceptance = "accept-from-all"
            fanout = "silent-relay"

            [execution]
            runs_per_experiment = 5
        "#
        .to_string()
    }

    // 016-FR-031: a well-formed description parses with resolved counts and
    // the publishes-per-run default.
    #[test]
    fn well_formed_description_parses() {
        let description = parse_sweep_description(&base_toml()).expect("valid description");
        assert_eq!(description.size, 20);
        assert_eq!(description.publishes_per_run, 1);
        assert_eq!(description.runs_per_experiment, 5);
        assert_eq!(description.model.name(), "m2");
        let resolved = description
            .resolved_experiments()
            .expect("validated at parse");
        assert_eq!(resolved.len(), 1, "no axes ⇒ one experiment");
        assert_eq!(resolved[0].adversarial, 2);
        // churn 0.1 of 18 honest → 2 (nearest).
        assert_eq!(resolved[0].churn_count, 2);
    }

    // 016-FR-028 / contracts/sweep-config.md: axes expand as a cross-product
    // in declaration order — the first-declared axis varies slowest — with
    // per-grid-point count resolution.
    #[test]
    fn axes_expand_as_a_cross_product_in_declaration_order() {
        let toml = base_toml()
            + r#"
            [[axes]]
            parameter = "churn"
            values = [0.0, 0.25]

            [[axes]]
            parameter = "target_degree"
            values = [3, 5]
        "#;
        let description = parse_sweep_description(&toml).expect("valid description");
        let resolved = description
            .resolved_experiments()
            .expect("validated at parse");
        assert_eq!(resolved.len(), 4);
        // churn varies slowest: (0.0,3), (0.0,5), (0.25,3), (0.25,5).
        // churn 0.25 of 18 honest → 4 or 5 (round(4.5)); pin the resolution.
        let grid: Vec<(usize, Option<usize>)> = resolved
            .iter()
            .map(|point| (point.churn_count, point.honest_strategies.target_degree))
            .collect();
        assert_eq!(grid[0], (0, Some(3)));
        assert_eq!(grid[1], (0, Some(5)));
        assert_eq!(grid[2].1, Some(3));
        assert_eq!(grid[3].1, Some(5));
        assert_eq!(grid[2].0, grid[3].0);
        assert!(grid[2].0 == 4 || grid[2].0 == 5, "0.25 of 18 honest");
        // The target-degree override lands on BOTH classes' tables.
        assert_eq!(resolved[1].adversarial_strategies.target_degree, Some(5));
    }

    // Axis validation: unknown parameters, duplicates, empty value lists,
    // wrong value shapes, and grid points that strand the population are all
    // rejected at parse — before any run executes.
    #[test]
    fn invalid_axes_are_rejected_at_parse() {
        let with_axis = |axis: &str| base_toml() + axis;
        assert!(matches!(
            parse_sweep_description(&with_axis(
                "[[axes]]\nparameter = \"gravity\"\nvalues = [1]\n"
            )),
            Err(SweepConfigError::UnknownAxisParameter { .. }),
        ));
        assert!(matches!(
            parse_sweep_description(&with_axis(
                "[[axes]]\nparameter = \"churn\"\nvalues = [0.0]\n\n[[axes]]\nparameter = \"churn\"\nvalues = [0.1]\n"
            )),
            Err(SweepConfigError::DuplicateAxis { .. }),
        ));
        assert!(matches!(
            parse_sweep_description(&with_axis("[[axes]]\nparameter = \"churn\"\nvalues = []\n")),
            Err(SweepConfigError::EmptyAxis { .. }),
        ));
        assert!(matches!(
            parse_sweep_description(&with_axis(
                "[[axes]]\nparameter = \"target_degree\"\nvalues = [3.5]\n"
            )),
            Err(SweepConfigError::AxisValueType { .. }),
        ));
        // The second grid point (churn_count 17 of 18 honest) leaves one
        // up-honest node — the whole sweep is rejected up front.
        assert!(matches!(
            parse_sweep_description(&with_axis(
                "[[axes]]\nparameter = \"churn_count\"\nvalues = [0, 17]\n"
            )),
            Err(SweepConfigError::TooFewUpHonest { remaining: 1, .. }),
        ));
    }

    #[test]
    fn unknown_model_is_rejected() {
        let toml = base_toml().replace("\"m2\"", "\"m9\"");
        assert!(matches!(
            parse_sweep_description(&toml),
            Err(SweepConfigError::UnknownModel(_)),
        ));
    }

    #[test]
    fn unknown_strategy_kinds_are_rejected() {
        let toml = base_toml().replace("\"silent-relay\"", "\"shout-relay\"");
        assert!(matches!(
            parse_sweep_description(&toml),
            Err(SweepConfigError::UnknownStrategy {
                seam: "fan-out",
                ..
            }),
        ));
        let toml = base_toml().replacen("\"uniform-sampler\"", "\"random-walk\"", 1);
        assert!(matches!(
            parse_sweep_description(&toml),
            Err(SweepConfigError::UnknownStrategy {
                seam: "connection",
                ..
            }),
        ));
    }

    #[test]
    fn uniform_sampler_requires_a_target_degree() {
        let toml = base_toml().replace("target_degree = 4\n", "");
        assert!(matches!(
            parse_sweep_description(&toml),
            Err(SweepConfigError::Strategy(_)),
        ));
    }

    #[test]
    fn conflicting_spellings_are_rejected() {
        let toml = base_toml().replace("churn = 0.1", "churn = 0.1\nchurn_count = 3");
        assert!(matches!(
            parse_sweep_description(&toml),
            Err(SweepConfigError::ConflictingFields { .. }),
        ));
        let toml = base_toml().replace(
            "adversarial = 2",
            "adversarial = 2\nadversarial_fraction = 0.1",
        );
        assert!(matches!(
            parse_sweep_description(&toml),
            Err(SweepConfigError::ConflictingFields { .. }),
        ));
    }

    // 016-FR-031: configurations leaving no up-honest publisher/receiver
    // pair are rejected at validation.
    #[test]
    fn populations_without_a_receiver_pair_are_rejected() {
        let toml = base_toml().replace("churn = 0.1", "churn_count = 17");
        assert!(matches!(
            parse_sweep_description(&toml),
            Err(SweepConfigError::TooFewUpHonest { remaining: 1, .. }),
        ));
        let toml = base_toml().replace("adversarial = 2", "adversarial = 19");
        assert!(matches!(
            parse_sweep_description(&toml),
            Err(SweepConfigError::TooFewUpHonest { .. }),
        ));
    }

    #[test]
    fn zero_counts_are_rejected() {
        for (from, to) in [
            ("size = 20", "size = 0"),
            ("runs_per_experiment = 5", "runs_per_experiment = 0"),
            (
                "runs_per_experiment = 5",
                "runs_per_experiment = 5\npublishes_per_run = 0",
            ),
        ] {
            let toml = base_toml().replace(from, to);
            assert!(
                matches!(
                    parse_sweep_description(&toml),
                    Err(SweepConfigError::ZeroCount { .. }
                        | SweepConfigError::TooFewUpHonest { .. }),
                ),
                "{to} must be rejected",
            );
        }
    }

    #[test]
    fn unknown_toml_fields_are_rejected() {
        let toml = base_toml().replace("master_seed = 42", "master_seed = 42\nresume = true");
        assert!(matches!(
            parse_sweep_description(&toml),
            Err(SweepConfigError::Parse(_)),
        ));
    }

    #[test]
    fn fractions_out_of_range_are_rejected() {
        let toml = base_toml().replace("churn = 0.1", "churn = 1.5");
        assert!(matches!(
            parse_sweep_description(&toml),
            Err(SweepConfigError::FractionOutOfRange { .. }),
        ));
    }
}

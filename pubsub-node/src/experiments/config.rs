//! Sweep-description parsing and validation: the loader layer between the
//! front-end binary's file/argument handling and the experiments API's
//! already-parsed values.
//!
//! Everything result-affecting lives in the TOML sweep description and is
//! validated here, before any run executes; invocation flags (output
//! directory, worker count, detail) stay on the binary and never reach the
//! manifest.
// 016-FR-031; contracts/sweep-config.md.

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

/// A parsed, validated sweep description: every result-affecting input,
/// with counts resolved (fractions applied) and strategy kinds checked.
#[derive(Clone, Debug)]
pub struct SweepDescription {
    /// The dissemination model.
    pub model: DisseminationModel,
    /// The sweep's master seed.
    pub master_seed: u64,
    /// Population size N.
    pub size: usize,
    /// Resolved adversarial count.
    pub adversarial: usize,
    /// Resolved churn count (honest nodes marked down per run).
    pub churn_count: usize,
    /// The single topic.
    pub topic: TopicId,
    /// Honest-class strategy configuration.
    pub honest_strategies: StrategyTable,
    /// Adversarial-class strategy configuration.
    pub adversarial_strategies: StrategyTable,
    /// Runs per experiment R.
    pub runs_per_experiment: u64,
    /// Publish phases per run (default 1).
    pub publishes_per_run: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSweepConfig {
    model: String,
    master_seed: u64,
    population: RawPopulation,
    strategies: RawStrategies,
    execution: RawExecution,
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
        (Some(count), None) => count,
        (None, Some(fraction)) => resolve_fraction(
            "population.adversarial_fraction",
            fraction,
            raw.population.size,
        )?,
        (None, None) => 0,
    };

    let honest = raw.population.size.saturating_sub(adversarial);
    let churn_count = match (raw.population.churn_count, raw.population.churn) {
        (Some(_), Some(_)) => {
            return Err(SweepConfigError::ConflictingFields {
                a: "population.churn_count",
                b: "population.churn",
            })
        }
        (Some(count), None) => count,
        // The churn proportion applies to the honest population — only
        // honest nodes churn.
        (None, Some(fraction)) => resolve_fraction("population.churn", fraction, honest)?,
        (None, None) => 0,
    };

    let remaining = honest.saturating_sub(churn_count);
    if adversarial > raw.population.size || remaining < 2 {
        return Err(SweepConfigError::TooFewUpHonest {
            size: raw.population.size,
            adversarial,
            churn: churn_count,
            remaining,
        });
    }

    // Probe both strategy tables now so a bad kind or parameter rejects the
    // sweep before anything runs.
    raw.strategies.honest.to_spec()?;
    raw.strategies.adversarial.to_spec()?;

    Ok(SweepDescription {
        model,
        master_seed: raw.master_seed,
        size: raw.population.size,
        adversarial,
        churn_count,
        topic,
        honest_strategies: raw.strategies.honest,
        adversarial_strategies: raw.strategies.adversarial,
        runs_per_experiment: raw.execution.runs_per_experiment,
        publishes_per_run,
    })
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
        assert_eq!(description.adversarial, 2);
        // churn 0.1 of 18 honest → 2 (nearest).
        assert_eq!(description.churn_count, 2);
        assert_eq!(description.publishes_per_run, 1);
        assert_eq!(description.runs_per_experiment, 5);
        assert_eq!(description.model.name(), "m2");
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

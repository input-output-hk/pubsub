use cardano/transaction.{OutputReference, Transaction}

validator dummy {
  spend(_d: Option<Data>, _r: Data, _u: OutputReference, _t: Transaction) {
    True
  }

  else(_) {
    fail
  }
}

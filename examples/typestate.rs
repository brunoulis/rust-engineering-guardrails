// A small, standard-library-only example. Compile with rustc --test and run.
// This is illustrative; state-specific types are optional for simpler workflows.
use std::marker::PhantomData;

#[derive(Debug)]
struct Unchecked;
#[derive(Debug)]
struct Validated;

#[derive(Debug)]
struct Command<State> {
    amount_cents: u64,
    _state: PhantomData<State>,
}

#[derive(Debug, PartialEq, Eq)]
enum ValidationError {
    ZeroAmount,
}

impl Command<Unchecked> {
    fn new(amount_cents: u64) -> Self {
        Self { amount_cents, _state: PhantomData }
    }
    fn validate(self) -> Result<Command<Validated>, ValidationError> {
        if self.amount_cents == 0 {
            return Err(ValidationError::ZeroAmount);
        }
        Ok(Command { amount_cents: self.amount_cents, _state: PhantomData })
    }
}

impl Command<Validated> {
    fn commit(self) -> u64 {
        self.amount_cents
    }
}

#[test]
fn validated_command_can_commit() {
    assert_eq!(Command::new(25).validate().unwrap().commit(), 25);
    assert_eq!(Command::new(0).validate().unwrap_err(), ValidationError::ZeroAmount);
}

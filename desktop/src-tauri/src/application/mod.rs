//! Application layer: the scheduler and the power-off gate.
//!
//! This is where the pure domain rules, the persistence seam, and the per-OS
//! platform adapters are joined into behaviour over time. It owns two things that
//! must have exactly one owner each:
//!
//! - every timer, so two timers can never fire for the same job ([`JobScheduler`]);
//! - the only call to a power-off, so the countdown cannot be bypassed
//!   ([`PowerOffGate`]).

pub mod grace_period;
pub mod scheduler;
pub mod transport;

#[cfg(test)]
pub mod fake_repository;

pub use grace_period::{
    grace_period_seconds, GraceClock, GraceOutcome, GraceState, InstantGraceClock, PowerOffGate,
    RealGraceClock, GRACE_PERIOD_SECONDS, REMOTE_GRACE_PERIOD_SECONDS,
};
pub use scheduler::{JobScheduler, SchedulerClock, SystemSchedulerClock};
pub use transport::{
    AccountId, AuthProvider, DevicePresenceRecord, FakeAuthProvider, FakeTransport,
    RemoteTransport, TransportFaults,
};

#[cfg(test)]
mod scheduler_tests;
#[cfg(test)]
mod transport_tests;

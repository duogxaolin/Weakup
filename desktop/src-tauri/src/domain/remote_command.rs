//! Whether a command arriving from another device is allowed to act on this one.
//!
//! Pure, and deliberately inert. This module decides; it does not act. An authorized
//! command is carried out by creating or modifying a *job*, which the target's existing
//! scheduler then runs by its existing rules — which is what keeps a remote power-off
//! inside the mandatory cancellable countdown. The countdown is reached because the job
//! path is the only path, not because the remote path chooses to be polite.
//!
//! So this file must never reach the gate or the executor, and that is enforced rather
//! than asked for: a source-text test in `remote_command_tests.rs` fails the build if
//! this file so much as names them. Rust's privacy rules would not catch it — the gate is
//! `pub` within the crate, so a future remote handler here could legally call it.
//!
//! The Dart implementation in `mobile/lib/domain/remote_command.dart` mirrors this file,
//! including the denial messages verbatim. `shared/testvectors/remote_authorization.json`
//! is what keeps the two from drifting.

use serde::{Deserialize, Serialize};

/// A command one device can ask another to carry out.
///
/// Closed rather than a string, so adding a fifth command is a compile error at every
/// match site — including [`RemoteCommand::is_remotely_permitted`], which is where a new
/// command would otherwise default to being accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteCommand {
    /// Schedule a power-off on the target. Reaches the machine as a job.
    PowerOff,
    /// Schedule a keep-awake on the target. Also a job; the remote path never acquires
    /// the assertion itself.
    KeepAwake,
    /// Cancel a scheduled job on the target, through its existing cancellation path.
    CancelJob,
    /// Turn on the target's remote-control setting. Never permitted remotely — see
    /// [`RemoteCommand::is_remotely_permitted`].
    EnableRemoteControl,
}

impl RemoteCommand {
    /// Whether this command is one a target accepts from another device at all.
    ///
    /// `EnableRemoteControl` is not, and no combination of pairing or settings makes it
    /// so. Requiring physical presence to grant the permission is what keeps access to
    /// an account from being enough to turn remote control on everywhere and then power
    /// off every device on it.
    pub fn is_remotely_permitted(self) -> bool {
        match self {
            Self::PowerOff | Self::KeepAwake | Self::CancelJob => true,
            Self::EnableRemoteControl => false,
        }
    }

    /// Whether carrying this command out requires the target to be able to power off.
    ///
    /// Per command rather than a blanket gate: cancelling a job asks nothing of the
    /// shutdown path, so a target that cannot power off still accepts a cancellation.
    pub fn requires_power_off_capability(self) -> bool {
        match self {
            Self::PowerOff => true,
            Self::KeepAwake | Self::CancelJob | Self::EnableRemoteControl => false,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PowerOff => "powerOff",
            Self::KeepAwake => "keepAwake",
            Self::CancelJob => "cancelJob",
            Self::EnableRemoteControl => "enableRemoteControl",
        }
    }

    pub fn from_str_value(value: &str) -> Option<Self> {
        match value {
            "powerOff" => Some(Self::PowerOff),
            "keepAwake" => Some(Self::KeepAwake),
            "cancelJob" => Some(Self::CancelJob),
            "enableRemoteControl" => Some(Self::EnableRemoteControl),
            _ => None,
        }
    }
}

/// Why a remote command was refused.
///
/// Closed rather than a string for two reasons: adding a fifth reason becomes a compile
/// error at every match site, and the shared vectors can name reasons exactly rather than
/// matching on prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DenialReason {
    /// The requesting device has not been paired with this one. Being signed in to the
    /// same account is not sufficient.
    NotPaired,
    /// The target's owner has not turned remote control on. This is every device's
    /// starting state.
    RemoteControlDisabled,
    /// The target cannot act as a remote target, or cannot perform this command.
    PlatformCannotPerform,
    /// No target accepts this command remotely, whatever else is true.
    CommandNotRemotelyAllowed,
}

impl DenialReason {
    /// Prose for the person who asked, in the style of [`AppError::user_message`].
    ///
    /// Specific per variant rather than a generic failure: a refusal the user cannot act
    /// on is indistinguishable from a bug. **These strings are byte-identical to the ones
    /// in `mobile/lib/domain/remote_command.dart`** — a user who sees a refusal on their
    /// phone and again on their laptop must not be told two different things.
    ///
    /// [`AppError::user_message`]: crate::core::AppError::user_message
    pub fn user_message(self) -> String {
        match self {
            Self::NotPaired => {
                "This device is not paired with that one. Pair them from the device you \
                 want to control."
                    .to_string()
            }
            Self::RemoteControlDisabled => {
                "Remote control is turned off on the target device. It can only be \
                 turned on at that device."
                    .to_string()
            }
            Self::PlatformCannotPerform => {
                "The target device cannot carry out that command. Phones and tablets \
                 cannot be controlled remotely."
                    .to_string()
            }
            Self::CommandNotRemotelyAllowed => {
                "That command cannot be sent from another device. It has to be done at \
                 the device itself."
                    .to_string()
            }
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotPaired => "notPaired",
            Self::RemoteControlDisabled => "remoteControlDisabled",
            Self::PlatformCannotPerform => "platformCannotPerform",
            Self::CommandNotRemotelyAllowed => "commandNotRemotelyAllowed",
        }
    }

    pub fn from_str_value(value: &str) -> Option<Self> {
        match value {
            "notPaired" => Some(Self::NotPaired),
            "remoteControlDisabled" => Some(Self::RemoteControlDisabled),
            "platformCannotPerform" => Some(Self::PlatformCannotPerform),
            "commandNotRemotelyAllowed" => Some(Self::CommandNotRemotelyAllowed),
            _ => None,
        }
    }
}

/// Accepted, or refused with exactly one reason.
///
/// A `(bool, Option<DenialReason>)` pair would admit two nonsense states — allowed with a
/// reason, and denied without one — and something would eventually construct one. Keeping
/// the reason inside `Denied` makes both unrepresentable, the same argument
/// `CalendarDate`'s validating constructor makes for February 30th.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteCommandDecision {
    Allowed,
    Denied(DenialReason),
}

impl RemoteCommandDecision {
    pub fn is_allowed(self) -> bool {
        matches!(self, Self::Allowed)
    }

    /// The refusal reason, or `None` when accepted.
    pub fn denial_reason(self) -> Option<DenialReason> {
        match self {
            Self::Allowed => None,
            Self::Denied(reason) => Some(reason),
        }
    }
}

/// Everything [`authorize`] needs, named.
///
/// A struct rather than five positional booleans: a row of booleans is a call site nobody
/// can read and a transposition nobody catches, and a transposition here authorizes a
/// command that should be refused.
///
/// The capability *booleans* are carried rather than a `PlatformCapabilities`. This rule
/// is evaluated for a *remote* device, from data that arrived over the wire;
/// `PlatformCapabilities::resolve()` describes the local machine. Passing the whole model
/// would invite calling `resolve()` inside the rule and silently authorizing against the
/// wrong device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoteCommandContext {
    /// What is being asked for.
    pub command: RemoteCommand,
    /// Whether the *target* can shut itself down.
    pub target_can_power_off: bool,
    /// Whether the *target* can act as a remote-control target at all.
    pub target_is_remote_target: bool,
    /// Whether the requesting device is paired with the target. Per device and
    /// revocable, not implied by both being on one account.
    pub is_paired: bool,
    /// Whether the target's owner has turned remote control on, at the target.
    pub remote_control_enabled: bool,
}

/// Decides whether the target accepts this command from this requester.
///
/// **The order of these checks is part of the contract**, and the shared vectors pin it:
/// `CommandNotRemotelyAllowed` → `RemoteControlDisabled` → `NotPaired` →
/// `PlatformCannotPerform`. Several reasons can hold at once, and an unspecified order
/// would let both implementations return "a" correct denial while disagreeing case by
/// case.
///
/// The order is not arbitrary: it discloses least. An unpaired requester learns only that
/// it is unpaired — not whether the target has remote control on, nor what the target is
/// capable of — so a caller cannot map an account's devices by reading refusal reasons.
/// Capability is checked last for exactly that reason.
pub fn authorize(context: &RemoteCommandContext) -> RemoteCommandDecision {
    if !context.command.is_remotely_permitted() {
        return RemoteCommandDecision::Denied(DenialReason::CommandNotRemotelyAllowed);
    }

    if !context.remote_control_enabled {
        return RemoteCommandDecision::Denied(DenialReason::RemoteControlDisabled);
    }

    if !context.is_paired {
        return RemoteCommandDecision::Denied(DenialReason::NotPaired);
    }

    if !context.target_is_remote_target
        || (context.command.requires_power_off_capability() && !context.target_can_power_off)
    {
        return RemoteCommandDecision::Denied(DenialReason::PlatformCannotPerform);
    }

    RemoteCommandDecision::Allowed
}

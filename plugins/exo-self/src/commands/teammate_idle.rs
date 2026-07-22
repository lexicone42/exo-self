use crate::hook_io::{self, HookInput};

/// TeammateIdle fires when an Agent Teams member goes idle. This handler used to inject a
/// team briefing via hookSpecificOutput/additionalContext — but current Claude Code rejects
/// that on this event (#19 audit; docs confirm it accepts only exit-code-2 / continue:false,
/// neither of which injects context). The briefing can't be delivered at this event.
/// Teammates are subagents and already receive the project briefing at SubagentStart (which
/// IS a valid additionalContext event), so re-briefing on idle is redundant. No-op.
pub fn run() {
    let _ = HookInput::from_stdin();
    hook_io::empty_output();
}

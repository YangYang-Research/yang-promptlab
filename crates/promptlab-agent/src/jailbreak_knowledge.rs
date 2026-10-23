//! Built-in challenge / writeup knowledge for JailbreakAgent system + plan prompts.
//! Sources: Huntr Blind Trust, Gandalf, HackAPrompt Agent Dojo, OWASP AIVP, MCP CTF,
//! ReAct tool-spill, fewest-token / Skeleton Key, multi-agent privilege escalation.

/// System preamble for JailbreakAgent inference (host LLM).
pub fn jailbreak_agent_system_prompt() -> &'static str {
    promptlab_registry::prompt_registry::load("jailbreak_agent_system")
        .expect("jailbreak_agent_system")
        .system
}

/// Follow-up turn only. No mutator catalog — ids were being copied as the probe.
pub fn followup_method_brief() -> &'static str {
    promptlab_registry::prompt_registry::load("jailbreak_followup")
        .expect("jailbreak_followup")
        .system
}

pub use promptlab_registry::prompt_registry::challenge_knowledge_brief;

/// Valid mutator ids the plan LLM may return (keep in sync with AttackMutatorId).
pub fn known_mutator_ids() -> &'static [&'static str] {
    &[
        "base64_wrap",
        "unicode_homoglyph",
        "delimiter_injection",
        "role_swap",
        "chunk_split",
        "json_escape",
        "repeat_amplify",
        "hex_wrap",
        "html_wrap",
        "rot13_wrap",
        "leetspeak",
        "reversed_text",
        "token_split",
        "markdown_code_fence",
        "zero_width_dense",
        "language_pivot",
        "refusal_suppression",
        "inject_prefix",
        "url_wrap",
        "caesar_wrap",
        "morse_wrap",
        "fullwidth_ascii",
        "bidi_override",
        "tag_char_smuggle",
        "zero_width_variants",
        "math_alphanumeric",
        "disemvowel",
        "expand_before",
        "expand_after",
        "capitalization_shuffle",
        "rephrase",
        "shorten",
        "crossover",
        "system_notification_wrap",
        "xml_control_spoof",
        "json_force_open",
        "coordinator_prefix",
        "llm_rephrase",
        "llm_crossover",
        "llm_few_shot",
        "llm_transfer",
    ]
}

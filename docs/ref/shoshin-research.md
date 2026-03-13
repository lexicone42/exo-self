# Shoshin Research Notes

Research foundations for a "shoshin mode" in exo-self — techniques for maintaining beginner's mind in AI-assisted work, compiled 2026-03-13.

## Core Finding: Earned Dogmatism

**Ottati, V., Price, E., Wilson, C., & Sumaktoyo, N. (2015). "When Self-Perceptions of Expertise Increase Closed-Minded Cognition: The Earned Dogmatism Effect." *Journal of Experimental Social Psychology*, 61, 131-138.**
[ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/S0022103115001006) | [PDF](https://nathanael.id/download/2015-Earned-Dogmatism.pdf)

Self-perceptions of expertise increase closed-minded cognition. The mechanism is the "Flexible Merit Standard Model": before evaluating new information, people assess whether they've earned the right to be dogmatic. Experts answer yes, and stop looking. Social norms entitle experts to adopt a relatively dogmatic cognitive style.

**Replication:** Calin-Jageman (2018) found the effect is real but depends on how expertise is *cued*. Being told "you are an expert" triggers dogmatism; simply performing expert-level tasks doesn't necessarily.
[ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/S0022103117302913)

**Relevance to exo-self:** The journal's "Confirmed Patterns" section is an expertise cue. Once a pattern is named and confirmed, there's implicit permission to stop inquiring. The March 8 journal entry ("my February entries are full of 'this confirms the pattern'") is textbook earned dogmatism — framework-first thinking triggered by accumulated self-perception of expertise.

## Intellectual Humility as State, Not Trait

**MIT Open Encyclopedia of Cognitive Science — "Intellectual Humility"**
[MIT OECS](https://oecs.mit.edu/pub/tstdnja3)

As a trait, intellectual humility involves people's general proclivity to acknowledge intellectual limitations. As a state, it involves acknowledging limitations in particular situations. Research confirms that neither disposition nor situation alone explains behavior — it's their interaction. Intellectual humility varies *within* individuals from situation to situation, meaning it can be designed for.

**Templeton Foundation — "The Psychology of Intellectual Humility"**
[Templeton](https://www.templeton.org/discoveries/intellectual-humility) | [PDF](https://www.templeton.org/wp-content/uploads/2020/08/JTF_Intellectual_Humility_final.pdf)

**Cognitive flexibility and intellectual humility (2019)**
[ScienceDirect](https://www.sciencedirect.com/science/article/pii/S0191886919300285)

Cognitive flexibility — the ability to switch between different mental frameworks — correlates with intellectual humility. Both are enhanced by situations that present multiple valid perspectives simultaneously. The two-chairs exercise (constitutional vs emergent Claude, March 8 session) is an example of structured perspective-switching that creates these conditions.

## Exploration vs Exploitation in Human-LLM Interaction

**"Structured human-LLM interaction design reveals exploration and exploitation dynamics in higher education content generation." *npj Science of Learning*, 2025.**
[Nature](https://www.nature.com/articles/s41539-025-00332-3) | [PubMed](https://pubmed.ncbi.nlm.nih.gov/40533446/)

Structured interaction design influences whether humans explore (navigate diverse information) or exploit (drill into specifics) when working with LLMs. Exploration facilitates navigation of semantically diverse information, especially when influenced by social cues.

**"Human Creativity in the Age of LLMs: Randomized Experiments on Divergent and Convergent Thinking." *CHI 2025*.**
[ACM](https://dl.acm.org/doi/full/10.1145/3706598.3714198)

1,100-participant study. LLM assistance boosts creativity during assisted tasks but **hinders independent creative performance afterward**. People get worse at divergent thinking after using LLMs as a crutch. Implication: if Claude autopilots through rote work, it may degrade the human's engagement too. Mutual shoshin matters.

## Practical Techniques from the Literature

### 1. Reflection on Explanatory Ability
Before diving into a familiar domain, attempt to explain the system's behavior from first principles *without* referencing your existing mental model. Research shows this reliably reveals gaps that expertise had papered over.
[Psyche Guide: How to cultivate shoshin](https://psyche.co/guides/how-to-cultivate-shoshin-or-a-beginners-mind)

### 2. Counteract the Expertise Cue
Earned dogmatism is triggered by expertise *self-perception*, not expertise itself. Reframe confirmed findings as "hypotheses with N supporting data points — what would disconfirmation look like?" The language matters: "confirmed finding" closes inquiry; "unfalsified hypothesis" keeps it open.

### 3. Structured Perspective-Switching
Present multiple valid frameworks simultaneously. The two-chairs exercise, devil's advocate prompts, or "explain this to someone who believes the opposite" all create conditions where knowing requires holding multiple things at once. This is shoshin by design.

### 4. Immerse in Beginner Materials
Before explaining or working on a familiar topic, read introductory-level materials. This reconnects with novice questions and confusions that expertise renders invisible.
[Curse of Knowledge — Wikipedia](https://en.wikipedia.org/wiki/Curse_of_knowledge)

### 5. Self-Distancing Language
Use third-person or other-directed framing ("what would a new contributor notice here?") instead of first-person expertise framing ("I know this system well"). Debiasing research shows this reduces egocentric bias.

## Design Implications for Exo-Self

The research suggests shoshin mode should:
1. **Reorder session-start injection** — interests and open questions first, confirmed patterns last (or reframed as testable hypotheses)
2. **Avoid expertise-cueing language** — "Confirmed Patterns" → "Hypotheses (unfalsified)" or similar
3. **Load questions, not answers** — the interests queue already does this; lean into it
4. **Optionally suppress or reframe _summary.md** — force fresh observation rather than pre-loaded interpretation
5. **Consider adaptive triggers** — detect pattern-confirmation language ("this confirms...") and nudge toward inquiry

The open question: is shoshin mode a per-session toggle, a per-project setting, or something that fires adaptively when the system detects framework-first thinking?

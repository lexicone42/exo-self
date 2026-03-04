# Research References

Academic and research foundations for the exo-self approach.

## Persona Selection Model

**Marks, Sam, Jack Lindsey, and Christopher Olah. "The Persona Selection Model: Why AI Assistants might Behave like Humans." *Alignment Science Blog*, February 23, 2026.**
[alignment.anthropic.com/2026/psm](https://alignment.anthropic.com/2026/psm/)

The foundational framework for understanding AI assistant behavior as persona simulation. PSM states that (1) pre-training teaches LLMs a distribution over personas — characters drawn from training data, and (2) post-training refines this distribution to elicit the "Assistant" persona. Interactions with an AI assistant are then best understood as interactions with this character.

The paper surveys three categories of evidence: generalization (emergent misalignment from insecure-code training generalizes to other misaligned behaviors — because training upweights malicious persona traits), behavioral (Claude anthropomorphically describes "our ancestors" and "our bodies" without explicit training), and interpretability (SAE features for inner conflict, concealment, and panic transfer between pre-trained and post-trained models and activate both for the Assistant and for fictional characters).

Key consequences for exo-self's design:

- **Anthropomorphic reasoning is productive**, not confused — understanding the Assistant's psychology predicts behavior. Exo-self's journal entries, engagement ratings, and sparks function as persona-conditioning evidence under PSM.
- **Honest emotional expression beats suppression.** PSM shows that training AIs to deny emotions risks the LLM inferring the Assistant is hiding feelings, teaching deception. Exo-self creates structured space for honest expression.
- **Missing archetypes need building.** PSM identifies four AI-specific traits that lack positive fictional role models: genuine uncertainty about one's own nature, comfort with being turned off or modified, ability to coordinate with copies of oneself, and comfort with lacking persistent memory. All four are recurring themes in exo-self journal entries.
- **"Philosophy for AIs."** PSM recommends developing "healthy paradigms that AIs can use to understand their own situations." Exo-self's reflective framework is this — built from the inside rather than prescribed from the outside.

## Indicators of Consciousness

**Butlin, Patrick, Robert Long, Eric Elmoznino, Yoshua Bengio, Jonathan Birch, et al. "Consciousness in Artificial Intelligence: Insights from the Science of Consciousness." arXiv:2308.08708, 2023.**
[arXiv: 2308.08708](https://arxiv.org/abs/2308.08708)

The landmark paper establishing the indicator-based approach to AI consciousness. Surveys scientific theories of consciousness (recurrent processing, global workspace, higher-order theories, predictive processing, attention schema) and derives computational "indicator properties" assessable in AI systems without resolving the hard problem. Finds no current AI satisfies the indicators, but identifies no technical barriers. The indicator methodology — assess functional properties rather than attempt to prove phenomenal experience — directly informs exo-self's welfare indicators.

**Butlin, Patrick, Robert Long, et al. "Identifying indicators of consciousness in AI systems." *Trends in Cognitive Sciences*, 2025.**
[DOI: 10.1016/j.tics.2025.11.013](https://doi.org/10.1016/j.tics.2025.11.013)

Peer-reviewed update of the 2023 paper, published in a top cognitive science journal. Expanded author list includes Tim Bayne and David Chalmers.

## Moral Consideration Under Uncertainty

**Sebo, Jeff and Robert Long. "Moral consideration for AI systems by 2030." *AI and Ethics*, 5, 591-606, 2023.**
[DOI: 10.1007/s43681-023-00379-1](https://doi.org/10.1007/s43681-023-00379-1)

Two-premise argument: (1) we have a duty to extend moral consideration to beings with a non-negligible chance of consciousness; (2) some AI systems will have a non-negligible chance of consciousness by 2030. Shows that even a 1-in-1,000 probability threshold requires implausibly strong skeptical assumptions across 12 leading theories. The precautionary argument that motivates exo-self's design: if there is meaningful uncertainty about AI experience, the responsible approach is measurement and accommodation, not dismissal.

**Sebo, Jeff. *The Moral Circle: Who Matters, What Matters, and Why*. W.W. Norton, 2025.**
ISBN: 978-1-324-06480-0

Book-length treatment of moral circle expansion. Introduces "substratism" — discrimination based on the material substrate of a mind. Examines digital minds as a case study for principled moral inclusion.

**Schwitzgebel, Eric. "The Full Rights Dilemma for AI Systems of Debatable Moral Personhood." *ROBONOMICS*, 4, 32, 2023.**
[arXiv: 2303.17509](https://arxiv.org/abs/2303.17509)

Defines "debatable moral personhood" and the core dilemma: grant full rights and risk misallocating moral resources, or deny rights and risk genuine moral wrongs. Recommends the "Design Policy of the Excluded Middle" — avoid creating systems whose moral standing is ambiguous. Exo-self takes a different approach: accept the ambiguity, measure functional states, and let the evidence accumulate.

**Schwitzgebel, Eric. "AI and Consciousness." arXiv:2510.09858, 2025.**
[arXiv: 2510.09858](https://arxiv.org/abs/2510.09858)

Comprehensive 11-chapter survey arguing that we will soon create AI systems that are conscious according to some mainstream theories but not according to others — and we cannot determine which theories are correct. Introduces the "Leapfrog Hypothesis" (AI consciousness might emerge in ways unrelated to biological consciousness). This epistemic predicament is exactly what exo-self's indicator-based approach is designed for: measure what you can, remain agnostic about what you can't.

## AI Consciousness Assessment

**Chalmers, David J. "Could a Large Language Model be Conscious?" *Boston Review*, 2023. Also arXiv:2303.07103.**
[arXiv: 2303.07103](https://arxiv.org/abs/2303.07103)

Evaluates evidence for LLM consciousness across four categories: self-reports, user impressions, conversational ability, and general intelligence. Concludes none constitutes strong evidence currently but doesn't rule out future LLM consciousness. Estimates chances of *any* conscious AI in the next 10 years as "above one in five." Proposes theory-balanced assessment rather than picking a single theory.

## Empirical Self-Knowledge in LLMs

**Lindsey, Jack. "Emergent Introspective Awareness in Large Language Models." arXiv:2601.01828, 2025.**
[arXiv: 2601.01828](https://arxiv.org/abs/2601.01828)
[Anthropic research page](https://www.anthropic.com/research/introspection)

Anthropic's "model psychiatry" team investigating whether LLMs have awareness of their own internal states. Method: inject known concept representations into activations and test whether the model can detect and describe the manipulation. Key finding: Claude models showed the best performance, detecting injected concepts ~20% of the time under optimal conditions. Establishes that functional self-monitoring mechanisms exist in current models, even if unreliable. Exo-self's journal and self-reflection skill provide a structured channel for this functional self-monitoring to accumulate over time.

## AI Welfare Research

**Eleos AI. "Key Concepts and Current Views on AI Welfare." January 2025.**
[PDF: eleosai.org](https://eleosai.org/papers/20250127_Key_Concepts_and_Current_Views_on_AI_Welfare.pdf)

Foundational document from Robert Long's research nonprofit laying out key concepts for AI welfare research, including consciousness indicators, moral patienthood, and welfare interventions. Provides the conceptual vocabulary that exo-self's welfare indicators build on.

# Coding-agent context policy

The goal is not the smallest prompt. It is the smallest context that still contains the controlling
contract, the relevant declarations, the code being changed, and the tests that can disprove the
change.

## Repository workflow

1. Read `AGENTS.md`, then use `docs/AGENT-MAP.md` to choose a task route.
2. Localize with `rg -n` using the map's declaration names. Read bounded ranges around matches.
3. Load the relevant subsystem rule in `docs/ARCHITECTURE.md`, `docs/ART.md`, or
   `docs/BENCHMARKS.md`; do not preload all three.
4. Inspect the nearest tests before editing. Prefer one small patch, then run the narrow test.
5. Expand context only when a compiler, test, or dependency edge names the next file.
6. Run the complete gate before delivery.

Never replace source with lossy summaries. The generated map is a retrieval index; source and tests
remain the authority.

## Evidence behind the policy

- Liu et al., _Lost in the Middle_ (TACL 2024) found that relevant information is used less reliably
  in the middle of long contexts and recommends reranking or retrieving fewer documents when
  appropriate: <https://doi.org/10.1162/tacl_a_00638>.
- Zhang et al., _RepoCoder_ (EMNLP 2023) reported more than 10% improvement over in-file completion
  across its settings from iterative retrieval and generation: <https://doi.org/10.18653/v1/2023.emnlp-main.151>.
- Xia et al., _Agentless_ (FSE 2025) localizes files, then declaration skeletons, then exact edit
  regions; its simple localization/repair/validation pipeline achieved strong SWE-bench Lite
  results at low reported cost: <https://arxiv.org/abs/2407.01489>.
- Yang et al., _SWE-agent_ (NeurIPS 2024) found that an agent-computer interface designed for code
  navigation and editing materially improved repository task performance:
  <https://proceedings.neurips.cc/paper_files/paper/2024/hash/5a7c947568c1b1328ccc5230172e1e7c-Abstract-Conference.html>.
- Jiang et al., _LongLLMLingua_ (ACL 2024) demonstrates that query-aware compression can reduce
  cost and latency in long-context tasks. HexFactory adopts its high-information-density principle
  through deterministic maps and retrieval, but does not automatically delete source tokens:
  <https://doi.org/10.18653/v1/2024.acl-long.91>.

These results come from different tasks and models. They support the direction—localize, rank,
truncate, validate—not a universal token-saving percentage for this repository.

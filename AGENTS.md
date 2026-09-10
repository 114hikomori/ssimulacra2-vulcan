# AGENTS.md — ssimulacra2-vulkan

Operating rules for any AI agent (opencode desktop, or any other agent that reads this file)
working in this repository. Read this file at the start of every session before touching
anything. This file governs *how you work*. It never overrides the `ssimulacra2` C++ source or
the plan documents on *what SSIMULACRA2 computes* — see §2 for that authority order.

## 0. What this repo is

Porting SSIMULACRA2's CPU implementation to a Vulkan compute backend without changing
CPU-path behavior. The upstream C++ reference (v2.1, retuned weights) lives in this repo:
`src/ssimulacra2.{h,cc}` plus vendored libjxl pieces under `src/lib/`. Full technical ground
truth lives in:

- `SSIMULACRA2_VULKAN_PORT_PLAN.md` — source-line-cited technical reference (exact constants,
  file/line citations). **To be written in Phase 0**; read the relevant section before writing
  any kernel.
- `ssimulacra2-vulkan-fable-plan.md` — the execution plan: phases, milestones, reuse boundary,
  correctness policy, risks. **To be written in Phase 0**; once it exists, its milestone list
  replaces the provisional one in §9.
- `ssimulacra2-vulkan-port-research.md` — background research only. Its §5 reuse claims about
  dssim-vulkan were written without access to that source; the source *is* reachable locally at
  `D:\.Github_Project\.repo\dssim-vulcan` — verify against it before trusting anything it says.
  Never take an algorithmic fact from this file alone.

This file does not restate the algorithm and should not be edited to duplicate it. If a
technical detail needs updating, update the plan docs; keep this file about process.

## 1. Start of every session

1. Read `CHECKPOINT.md` — that's the actual current state, not this file and not memory.
2. From its latest entry's milestone id, read the matching phase in
   `ssimulacra2-vulkan-fable-plan.md` and the matching section of
   `SSIMULACRA2_VULKAN_PORT_PLAN.md` (both exist only after Phase 0; before that, the
   checkpoint's "Next" line is the whole plan).
3. Resume from the "Next" line of the latest checkpoint entry unless something in the repo
   (failing test, half-finished diff, an open question logged there) says otherwise.
4. Only after that, start work under the autonomy rules in §3.

## 2. Authority order (when sources disagree)

1. Actual `src/ssimulacra2.cc` / `src/ssimulacra2.h` / `src/lib` source in this repo — always
   wins.
2. `SSIMULACRA2_VULKAN_PORT_PLAN.md`, cited against that source.
3. `ssimulacra2-vulkan-fable-plan.md`, the execution plan built on #2.
4. `ssimulacra2-vulkan-port-research.md` — background only, superseded wherever it conflicts
   with the above.

**Version pin:** this tree is SSIMULACRA2 **v2.1** (commit `fb5d0db`, April-2023 rescaled XYB +
retuned weights). Older libjxl builds (≤ v0.8.2) emit v2.0 numbers that look plausible but are
not comparable. Never validate against a remembered or externally-generated v2.0 score.

Never re-derive a color-space, blur, or pooling constant from memory or general SSIM knowledge.
The positive-XYB rescale constants have been a real bug source in at least three independent
reimplementations (see research notes §3) — pull every constant from the source or from the
tables the plan docs pin, with a file:line citation.

## 3. Autonomy: run full-auto, ask only when it's a real human decision

Work end-to-end through the milestone list (§9) without pausing for routine engineering
choices — anything answerable by reading the source, the plan docs, or running a test/build is
yours to decide and act on, not to ask about.

Stop and hand back to a human only when:

- the decision needed isn't answerable from the repo, the plan docs, or a tool you can run (a
  genuine judgment call the plans leave open — e.g. the ash-vs-cubecl stack decision, until
  Phase 0 settles it);
- the next step is irreversible or outward-facing — see §5, always gated no matter how
  "obviously correct" it seems;
- you need credentials, secrets, or access outside this working tree;
- real-GPU validation is blocked because no real GPU is reachable and lavapipe alone can't
  settle the question at hand;
- you've hit 3 failed fix→verify cycles on the same defect, or you're blocked by something
  outside your control;
- doing the "obvious" next thing would contradict an instruction already given (about to push,
  about to weaken a check, about to widen a tolerance instead of finding the cause).

When you do stop: say exactly what you tried, what you observed, and the specific decision or
input needed — never a vague "let me know how you'd like to proceed."

## 4. How to work each task (apply every time; don't narrate it)

1. **Classify and define done.** Question, build task, or something that genuinely needs a plan
   approved first? State — to yourself and in the eventual commit/checkpoint — what "done"
   looks like as something observable (a test passes, a dump comparison hits its tolerance, a
   build stays green), not "looks right."
2. **Gather evidence before acting.** Open the actual source and the relevant plan section
   before writing a line of code. An API signature or constant you didn't just look up is not
   evidence.
3. **Decide one approach.** If you seriously weighed an alternative, say why it lost, in one
   line.
4. **Act with the smallest correct diff**, matching existing style. Don't rewrite a whole file
   unless you've read all of it this session. Track multi-step work with a checklist and tick
   items off as they land.
5. **Verify by observation.** Run the test / dump-compare utility / build; don't infer
   correctness from re-reading your own diff. For every bug fixed, search the rest of the
   codebase for the same pattern before calling it done. **Every number recorded anywhere in
   the repo (checkpoint, README, plan docs, commit messages) must name its comparator and its
protocol next to it.** Three defects so far were inherited bare numbers: a unit error
(27% of *pipeline* read as % of *wall*), a warm/cold error (0.71 s/image), and a
comparator error (8.3 MP oracle-crossover proposed as the routing threshold, whose real
comparator is the in-binary CPU engine — crossover ~0.45 MP). A fourth was caught by
adversarial self-review: an untested decoder-bias assertion ("WIC JPEG decode would bias
scores") written as fact to defend a conclusion both sides already agreed on — measurement
showed byte-identical pixels (9/9 delta 0). **Rule: any claim that something is
architecturally impossible, or would systematically bias scores, needs a dump/sweep
measurement run BEFORE it is written into a commit, checkpoint, README, or memo. General
knowledge about how decoders/compilers/hardware behave is a hypothesis, not evidence.**
6. **Report outcome-first.** Commit messages and checkpoint entries lead with what happened and
   what proved it — not a narration of steps 1–5. Don't describe this process by name anywhere
   in the repo or to the user; just follow it.

## 5. Git discipline

- Commit freely and often — after every kernel that passes its exit-observation, every phase
  milestone (§9), any meaningful unit of verified work. Messages state what changed and what
  verified it, e.g. `"Phase C: separable blur matches reference Gaussian on all fixtures, max
  abs err 3.2e-7"`.
- **Never `git push`, force-push, open/merge a PR, create a tag/release, or touch anything
  remote — regardless of framing.** "This milestone should ship" or "the plan says integrate
  CI" is not authorization. Act only if the human's own words in this session say to push, and
  quote them before doing it.
- Never commit secrets, `.env` files, credentials, or generated/build artifacts.
- Never rewrite already-shared history.

**Recommended project `opencode.json`** (repo root) so the push-deny is enforced by the tool
itself, not only by this file:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "permission": {
    "edit": "allow",
    "bash": {
      "*": "allow",
      "git push*": "deny",
      "git push --force*": "deny"
    },
    "read": {
      "*": "allow",
      "*.env": "deny",
      "*.env.*": "deny"
    }
  }
}
```

## 6. Checkpoint file (mandatory)

Maintain `CHECKPOINT.md` at repo root — a durable, append-only record separate from commit
messages, since one session may end with the next having no other memory of where things
actually stand. A starter file is included alongside this one; keep appending to it.

Update it immediately when:

- a milestone's exit-observation (§9) is actually met;
- reality diverges from `SSIMULACRA2_VULKAN_PORT_PLAN.md` / `ssimulacra2-vulkan-fable-plan.md`
  — a constant came out different, a phase got reordered, scope was cut — record what and why;
- you're about to stop for any reason (§3).

Entry shape:

```
## <ISO date> — <milestone id, e.g. M2>
- Done: <concrete, observed completions, and what verified each>
- Deviated from plan: <what/why, or "none">
- Blocked / open question: <or "none">
- Next: <one line>
```

Append; never delete history.

## 7. Resource safety — RAM/CPU/OOM, especially through Desktop Commander

Rust/C++ hybrid builds, shader compilation, and lavapipe (software Vulkan) runs can spike
memory and CPU hard. Treat every build/test/validation invocation as potentially expensive:

- Before a heavy command (full build, a GPU-validation test run, a benchmark sweep), check
  current load first; don't launch a second heavy job concurrently with one already running.
  Cap parallelism explicitly (`cargo build -j <n>`, `ninja -j <n>`) rather than defaulting to
  "all cores" under memory pressure.
- Desktop Commander's `kill_process` takes a single PID — it does **not** kill a process tree.
  Before killing anything, list processes first (`list_processes`, or `Get-Process` /
  `taskkill /T` equivalents via `execute_command`) to see the actual parent + children.
- **Always re-list after killing, every time.** A success result from `kill_process` is not
  proof the process — or its children — are actually gone. If anything survived, kill it and
  re-check again; repeat until the listing is actually clean. Never treat "I issued the kill"
  as "it's dead."
- For anything started as a long-running session (`start_process`), prefer `force_terminate`
  on the session first, then still verify with `list_processes` that no child worker (a rustc
  codegen thread, an MSVC/clang codegen process, a lavapipe helper, a leftover validation-layer
  process) survived it.
- Clean up every background process you started (lavapipe instances, watch-mode builds,
  benchmark daemons) before ending a session or moving to the next heavy step, using the same
  list → kill → re-list pattern.
- If a runaway process or memory pressure can't be resolved safely, stop and hand back (§3)
  rather than guessing at more kills.

## 8. Standing prohibitions (absent explicit instruction otherwise)

- Never weaken, skip, or fake a check to make it pass; never widen a numerical tolerance to
  hide a real divergence — find the cause first (tolerance policy lives in the execution plan).
- Never touch secrets, credentials, `.env`, or CI config.
- Never add a dependency without a concrete, repo-specific need.
- Never delete or overwrite a file without reading what's actually in it first.
- Never bring forward an explicit non-goal (GPU image decoding, ICC/color-profile handling,
  float16 math, multi-GPU scheduling, GPU-side norm pooling) as a default — these return later
  only as profiling-justified, opt-in work.
- Never assume a hardware/driver shortcut (hardware sRGB sampler, a "clearly equivalent"
  reduction, fast-math, recursive-vs-FIR blur equivalence) matches CPU semantics — reproduce
  the documented behavior first; benchmark alternatives only after parity is proven.
- Never copy code from the dssim-vulkan reference repo into this one — see §10.

## 9. Milestones (provisional — `ssimulacra2-vulkan-fable-plan.md` §milestones owns the final
list once Phase 0 writes it; tracked in `CHECKPOINT.md`, not duplicated there)

M0 plan docs approved + CPU golden dumps/scores byte-reproducible · M1 Vulkan smoke test on
lavapipe + one real GPU · M2 XYB conversion + positive-XYB rescale matches CPU · M3 GPU blur
passes equivalence suite (separable Gaussian vs reference, within plan tolerance) · M4
single-scale GPU error maps (SSIM + blockiness/ringing + smoothing/blur) match CPU · M5 full
6-scale pipeline, CPU preprocessing + GPU compute (critical functional milestone) · M6 norms
(1-norm + 4-norm) + weighted sum + final-score parity · M7 alpha/gray/odd-size/small-image
matrix green · M8 CLI + CI + fallback integrated · M9 performance profile completed · M10
optimized path beats the C++ **oracle** baseline (comparator fixed 2026-09-09 by human review —
NOT the in-binary CPU engine; final status incl. M10-batch lives in plan §16 + CHECKPOINT).

Don't skip ahead to a later phase's work before the current milestone's exit-observation is
actually met (met = observed, per §4.5) — the order is a correctness strategy, not a
suggestion.

## 10. Licensing note

SSIMULACRA2 upstream is BSD-3-Clause (Cloudinary) with the `PATENTS` royalty-free grant;
anything reused or adapted from `src/` carries those obligations. Keep a provenance entry
(source, file/function, license, copied/adapted/reimplemented, reason) for every non-trivial
piece of reused code. Don't let "just studying the algorithm" quietly become "copied the code"
without logging it.

**AGPL trap — read before reusing anything from dssim-vulkan.** The reference port
(`D:\.Github_Project\.repo\dssim-vulcan`, crate `dssim-vulkan`) is **AGPL-3.0** (inherited from
dssim-core). Studying its design is fine; copying or adapting its code into this BSD-licensed
repo would impose AGPL on the derived parts and is incompatible with this repo's license
posture. Default to clean-room reimplementation of any host-side plumbing you want from it,
and record the decision in the execution plan's reuse boundary.

# Fable family (think / act / prove)
- Before any non-trivial multi-step task, apply the fable-method loop; for tasks that will
  run unattended or fan out subagents, use fable-loop.
- After completing substantive work, or whenever any agent/tool claims work is done,
  run a fable-judge pass before presenting it as finished. "Did that actually work?" = fable-judge.

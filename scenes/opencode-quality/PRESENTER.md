# Spend tokens once. Keep the protection.

**OpenCode quality loops — team proposal · 02 October 2026**

## The decision

Approve a two-week pilot with one accountable maintainer, an agreed model-spend
ceiling, five issue clusters to assess, and three durable regression targets.
Start with existing test and operator tooling. Continue only if the work produces
credible, reusable evidence at an acceptable model and reviewer cost.

This is a proposal, not an announcement of fixes or a measured ROI result.
The principal audience is an OpenCode maintainer deciding where recurring agent
effort should go. The job is to make that allocation decision with clear tradeoffs.

## 1. Spend tokens once. Keep the protection.

The strongest use of recurring agent effort is not a recurring opinion about the
repository. It is discovering and preserving a contract that protects a user.
An agent can investigate a confusing report, construct the smallest faithful
fixture, and connect a proposed repair to its intended outcome. That work may be
expensive once. Subsequent checks should become ordinary test execution.

Three priorities reinforce each other: turn real reports into regressions; stress
the transitions where an otherwise working system can fail; and inspect the
actual user experience. The pitch is not “use more tokens.” It is “make a greater
fraction of the work survive the session that produced it.”

## 2. Users experience journeys, not modules.

A user does not experience a provider adapter, a session query, and a migration
as separate engineering achievements. They experience starting work, staying in
work, and returning to work. The boundary between components can break that
journey while individual component tests remain green.

This is not an argument against unit tests. Those tests give fast, precise
feedback. It is an argument for a narrow set of cross-boundary checks chosen from
actual failure reports: installation to connection, submission to recovery, and
upgrade to reopening an existing session. Prioritize by interrupted workflow,
not by how much code an agent can inspect.

## 3. Three reports. Three different blind spots.

The earlier issue review sampled 60 recent open and 40 recently closed reports.
44 of those 60 open reports were unlabeled. This is a bounded, non-random sample;
it does not estimate defect prevalence or prove labeling causes slow resolution.
It suggests that a triage loop should read behavior, version, and environment
rather than treating labels as the complete intake mechanism.

Three useful starting examples:

- [#52844 — migrated sessions missing from the TUI list](https://github.com/anomalyco/opencode/issues/52844).
  The reporter describes existing rows that are not discoverable through the
  TUI's project/subpath query after upgrade. Merely counting migrated rows or
  listing sessions through a different CLI query is a weaker check.
- [#52736 — desktop launch allocation loop](https://github.com/anomalyco/opencode/issues/52736).
  The reporter connects startup failure to persisted workspace scopes. A clean
  profile is useful as a control but does not represent an accumulated profile.
  Do not copy the report's data-deletion workaround into an automated repair.
- [#52758 — strict MCP initialization rejection](https://github.com/anomalyco/opencode/issues/52758).
  The reporter describes an initialization capability that a strict Java peer
  rejects before authentication. A forgiving mock can hide that contract failure.

These are **read reports, not independently reproduced findings from this deck**.
The proposal is to investigate and earn that stronger evidence. Selected reports
were revisited during authoring; their current status is not proof of resolution.

## 4. Do not rent the same insight twice.

A review that ends with prose has an expiration date: the next agent or reviewer
must reconstruct the conditions, reasoning, and confidence. A reproducible check
retains a portion of that investigation in an executable form. It makes a future
change confront the same user contract without a fresh model investigation.

The loop is report → failing proof → scoped repair → routine replay. A new
failure in replay starts a new investigation; a healthy replay does not need
another agent to explain the whole repository. “No model calls” does not mean
free: execution time, fixture upkeep, CI capacity, and review still matter.
The pilot should measure those costs rather than hiding them.

## 5. Require evidence on both sides of a repair.

The intended positive assertion must fail on the pinned base for the intended
reason. An unavailable dependency, broken setup, or unrelated exception is not a
reproduction. Then the same assertion must pass on the candidate, along with
controls for behavior we already want to preserve.

Preserve the replay command, base and candidate revisions, fixture identity,
ordering or seed, relevant assertion, and cleanup result. Explain whether the
proof exercises a domain function, a module with real filesystem operations, a
child process, full server boot, or the original interface. These are distinct
levels of evidence, not interchangeable labels.

The existing managed-service reliability work already uses this discipline.
The pilot should extend its useful assets rather than inventing a second proof
system. A merged or released patch is still distinct from a user reporting that
their original workflow now works.

## 6. A migration is not done when rows copy.

For #52844, propose a synthetic, disposable old-profile fixture containing root
sessions, subdirectory sessions, and the missing or legacy-path variants the
report describes. Run the actual migration. Then use the same project/subpath
query behavior as the TUI, find the expected sessions, and reopen one to verify
its content and location.

This is convincing because it tests the user's acceptance criterion, not just
the mechanism we happen to be changing. The new check should first reproduce
the intended failure on a selected base. Do not state that this fixture or a
repair already exists: the deck describes the test we would build.

## 7. Break the transition. Preserve the promise.

Transition stress should be contract-driven, not indiscriminate chaos. Select
one boundary: ownership while two starters race; acknowledgement loss around
prompt submission; server disappearance during bootstrap; or an upgrade between
compatible generations of data and clients. Define the observable promise first.

Use ordering barriers where feasible so the interesting interleaving actually
happens. Seeds help replay and explore; random-run volume does not prove that a
required fault boundary was reached. Minimize novel failures into stable cases.
Bound the deadline, record actual cleanup, and terminate only processes the
fixture owns. Never inject faults into a developer's live service or profile.

For prompt retry, first agree on what the delivery contract actually promises
and what the relevant ID deduplicates. Do not assume “exactly once” for arbitrary
external tool effects simply because a prompt admission ID is stable.

## 8. A green check can still miss the bug.

Three false-confidence traps deserve explicit attention:

1. An explicit-server TUI scenario can bypass managed service discovery and
   election. Useful UI evidence does not automatically become ownership proof.
2. Controlled health-response simulations exercise real child processes and
   sockets but are not the same as full application boot. Existing campaign
   records describe 100 passing cases over ten seeds on a candidate, with recorded
   base failures and controls. Those runs were **not rerun for this presentation**.
   Their counts should not be presented as 100 end-to-end product scenarios.
3. One local platform does not prove native Windows/WSL network classification
   or packaged desktop behavior. Keep those platform results separate.

[PR #50825](https://github.com/anomalyco/opencode/pull/50825) is a concrete
example of a scoped reconnect-preservation contract with before/after proof and
controls. It was **open when checked during authoring**. Do not call it shipped.

## 9. Make “shiny” a repeatable check.

Use the same fixture, viewport, theme, and state for before/after comparisons.
Include the awkward states: long content, narrow terminals, active focus,
scrolling, pending prompts, and dialog interruption. Assertions for clipping,
focus, or layout can be deterministic; final judgment about taste remains human.
Agents should flag a specific visual or interaction problem, not issue generic
style instructions.

A weekly newcomer exercise answers a different question: can someone using only
public documentation accomplish the setup journey on the packaged product?
Start with a clean profile, prohibit insider instructions, and record every
undocumented workaround. Test a bounded journey, not every provider every week.
The setup exercise complements accumulated-profile upgrade tests rather than
replacing them.

## 10. Closed is not the same as recovered.

Keep evidence states distinct: reported, reproduced, repaired on a candidate,
released, and user-confirmed recovery. Mitigation and an explanation are also
different outcomes. Clustering reports by symptom can help intake, but merging
symptoms into one root cause requires evidence.

GitHub owns the public user problem. Organizer owns private campaign tasks.
The repository owns executable protection. Cross-link them rather than copying
the same backlog into three places. A short brief should tell maintainers what
failed, what is protected, and what requires a decision. External replies and
issue closures remain deliberate, authorized human actions.

## 11. Parallelize problems—not opinions.

Separate reproduction, repair, and verification when the risk warrants it.
The reproducer establishes the contract and evidence boundary. The fixer owns a
narrow change. The verifier reruns the unchanged proof and asks whether healthy
controls or the original journey were bypassed. Multiple agents agreeing is not
a substitute for those checks, and separate roles do not guarantee independent
reasoning.

Parallelize genuinely independent issue clusters. Set a time, token, and attempt
budget before starting. If the same failure repeats without new evidence,
escalate or stop. Avoid an unattended repair loop that repeatedly changes the
test to accommodate the latest patch.

## 12. Use models where model behavior matters.

Stable recorded or scripted responses are appropriate for transport, lifecycle,
UI, and state contracts. They isolate the product behavior being tested. They
cannot establish that a real agent still solves a coding task, follows steering,
uses tools sensibly, or retains important context across compaction.

Keep a small separate real-model suite with pinned task fixtures, allowed tools,
budgets, and outcome checks. Include repeated trials where variance matters.
Score the completed task and relevant safety constraints rather than just the
similarity of a transcript. Expand this suite only when its signal justifies its
cost. [Anthropic's agent evaluation guidance](https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents)
is a useful methodological reference, not evidence of our projected savings.

## 13. Concentrate effort before expanding scope.

Start with the three loops closest to concrete user pain and lasting protection.
Add backlog audits, real-model tasks, and mutation tests selectively. A mutation
test asks whether a small deliberate defect makes the intended test fail; it can
reveal an assertion that passes while its contract is broken. Mutation across
the entire repository would be a different and more expensive proposal.

The default should not be a recurring whole-codebase quality essay, several
agents evaluating the same patch, or a new generic orchestration framework.
Those can produce activity without preserving a new contract. The rankings are
expected-return judgments. We have not measured OpenCode-specific ROI.

References: [fast-check model-based testing](https://fast-check.dev/docs/advanced/model-based-testing/)
and [Google Research's practical mutation testing](https://research.google/pubs/practical-mutation-testing-at-scale-a-view-from-google/).

## 14. Five clusters. Three durable proofs.

In week one, rank five issue clusters by blocked workflow, recurrence evidence,
reproducibility, and expected fixture cost. Choose three proof targets. Capture
the intended base failures and working controls. The three examples in this
deck are candidates, not commitments to their alleged root causes.

In week two, test available candidates or route a narrowly scoped repair through
ordinary review. Run one docs-only newcomer exercise and matched UI comparisons
where relevant. Retain a credible failing regression even if a repair cannot
ship during the pilot; that is useful evidence but not recovered user value.

The maintainer gets one short daily brief. Log model spend, reviewer minutes,
flakiness, cleanup problems, and unsuccessful attempts. Agree on the actual
ceiling before launching. Three durable proofs are a target; three shipped fixes
are not promised. The pilot is testing the workflow as much as the product.

## 15. Prove the loop pays for itself.

At day 14, report both sides of the ledger. What intended failures did the proofs
actually expose? Which user workflows have verified repairs? What can another
maintainer replay without the author? What did model usage and reviewer time
cost? Include maintenance and abandoned work rather than counting only wins.

Continue if the targeted proofs are credible, stable, retained, and affordable.
Narrow or stop if the results are mostly prose, brittle fixtures, scope growth,
or review burden. A two-week pilot is too short to establish long-term recurrence
reduction; track that afterward. Do not substitute agent activity, test count,
or lower token use for user outcomes.

The requested decision is modest: approve the pilot, name the maintainer, and
set the spend ceiling. No infrastructure investment is needed before the first
useful regression demonstrates the idea.

## 16. The starting point is not zero.

Local source inspection found client service tests, app visual-stability tooling,
timeline scenario matrices, and AI transport recording fixtures. The operator
workflow also has Drive for real TUI behavior, while the existing service proof
campaign supplies process-fault and contract-oriented examples.

Relevant paths in the inspected checkout include:

- `packages/client/test/service.test.ts`
- `packages/app/e2e/utils/visual-stability/`
- `packages/app/e2e/performance/timeline-stability/`
- `packages/app/e2e/regression/`
- `packages/desktop/src/renderer/startup/initialization.test.ts`
- `packages/ai/test/fixtures/recordings/`

This is an inventory, not a coverage audit or a claim that these suites are green.
Check the chosen contract and existing fixtures first. Add the missing observable
boundary instead of a broad second framework. Some campaign fixtures are local
work, not yet a reusable main-branch facility; integration and maintenance
ownership must be confirmed during the pilot.

## 17. Strong proposal. Honest evidence limits.

The slide deck combines an earlier bounded issue review, selected report and PR
reads, recorded internal service-campaign evidence, local source inspection,
and external testing methods. Those have different confidence levels.

No OpenCode repairs, reproductions, CI runs, issue comments, tracker edits, or
releases were performed to create this presentation. Psychopomp validation and
rendering verify the presentation artifact only. No measured cost saving or
universal coverage result is claimed. Private incident logs, database contents,
credentials, and private campaign identifiers are intentionally excluded.

The persuasive claim is therefore a practical one: we have concrete pain signals,
useful existing assets, a way to retain evidence, and a small experiment that
can tell us whether this allocation of agent effort deserves to grow.

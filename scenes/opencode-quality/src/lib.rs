//! A team proposal, not a report of completed OpenCode repairs.
//! Uses the native showroom's bounded typography and quiet Stage diagrams.
use anyhow::Result;
pub mod video;
use psychopomp::{
    author::{ActorHandle, PlanBuilder, SECOND},
    component_prototype::{Font, RICH_TEXT, RichTextPlan, TYPESET, TextPart, TypesetPlan},
    effects::spinner::Mark,
    plan::{DeckPlan, SlidePlan, SpringPlan},
    stage::{STAGE_RECIPE, StageElement, StagePlan, StagePost},
    tone::Tone,
};

const BEAT: u64 = 3 * SECOND;
const INK: [u8; 3] = [235, 233, 227];
const MUTED: [u8; 3] = [127, 133, 141];
const ACCENT: [u8; 3] = [216, 168, 120];

fn label(
    p: &mut PlanBuilder,
    id: &str,
    value: &str,
    at: [f32; 2],
    size: f32,
    color: [u8; 3],
) -> Result<()> {
    p.actor(
        id,
        TYPESET,
        TypesetPlan {
            origin: at,
            font: Font::Sans,
            font_size: size,
            parts: vec![TextPart {
                id: "text".into(),
                text: value.into(),
                color,
                spans: vec![],
            }],
            visible: vec!["text".into()],
            events: vec![],
        },
    )?;
    Ok(())
}

fn prose(
    p: &mut PlanBuilder,
    id: &str,
    value: &str,
    at: [f32; 2],
    width: f32,
    size: f32,
) -> Result<ActorHandle> {
    Ok(p.actor(
        id,
        RICH_TEXT,
        RichTextPlan {
            origin: at,
            width,
            font_size: size,
            markdown: value.into(),
            vertical_mask: None,
            fade_blur: 0.,
        },
    )?)
}

fn reveal(
    p: &mut PlanBuilder,
    actor: &ActorHandle,
    property: &str,
    step: usize,
    steps: usize,
    duration: f32,
) {
    let values: Vec<_> = (0..steps)
        .map(|i| if i >= step { 1. } else { 0. })
        .collect();
    p.step_track(actor, property, &values, SpringPlan::visual(duration, 0.));
}

fn page(
    index: usize,
    tag: &str,
    title: &str,
    subtitle: &str,
    steps: &[&str],
    source: &str,
) -> Result<PlanBuilder> {
    let mut p = PlanBuilder::new(format!("quality-{index:02}"), steps.len() as u64 * BEAT);
    for (i, description) in steps.iter().enumerate() {
        let at = i as u64 * BEAT;
        p.presentation_step(
            format!("step-{i}"),
            *description,
            at,
            if i == 0 { 0 } else { at + 2 * SECOND },
        );
    }
    label(&mut p, "brand", "opencode", [140., 60.], 28., INK)?;
    label(&mut p, "tag", tag, [140., 112.], 20., ACCENT)?;
    prose(
        &mut p,
        "title",
        &format!("**{title}**"),
        [140., 165.],
        1600.,
        54.,
    )?;
    prose(&mut p, "subtitle", subtitle, [145., 290.], 1600., 29.)?;
    label(&mut p, "source", source, [140., 1010.], 18., MUTED)?;
    label(
        &mut p,
        "page",
        &format!("{index:02} / 17"),
        [1650., 1010.],
        20.,
        MUTED,
    )?;
    Ok(p)
}

fn columns(
    index: usize,
    tag: &str,
    title: &str,
    subtitle: &str,
    content: &[&str],
    takeaway: &str,
    source: &str,
) -> Result<SlidePlan> {
    let descriptions: Vec<_> = content
        .iter()
        .enumerate()
        .map(|(i, _)| format!("Reveal recommendation {}", i + 1))
        .collect();
    let steps: Vec<_> = descriptions.iter().map(String::as_str).collect();
    let mut p = page(index, tag, title, subtitle, &steps, source)?;
    let stride = 1680. / content.len() as f32;
    for (i, value) in content.iter().enumerate() {
        let x = 145. + i as f32 * stride;
        label(
            &mut p,
            &format!("number-{i}"),
            &format!("0{}", i + 1),
            [x, 395.],
            23.,
            ACCENT,
        )?;
        let actor = prose(
            &mut p,
            &format!("column-{i}"),
            value,
            [x, 447.],
            stride - 70.,
            30.,
        )?;
        reveal(&mut p, &actor, "opacity", i, steps.len(), 0.16);
    }
    let actor = prose(
        &mut p,
        "takeaway",
        &format!("**{takeaway}**"),
        [145., 875.],
        1600.,
        30.,
    )?;
    reveal(
        &mut p,
        &actor,
        "opacity",
        steps.len() - 1,
        steps.len(),
        0.16,
    );
    Ok(SlidePlan {
        title: title.into(),
        plan: p.finish()?,
    })
}

fn flow(
    identity: (usize, &str),
    title: &str,
    subtitle: &str,
    nodes: &[&str; 4],
    notes: &[&str; 4],
    takeaway: &str,
    source: &str,
) -> Result<SlidePlan> {
    let (index, tag) = identity;
    let mut p = page(index, tag, title, subtitle, notes, source)?;
    let diagram = p.actor(
        "flow",
        STAGE_RECIPE,
        StagePlan {
            post: StagePost {
                bloom: 0.,
                grain: 0.,
                vignette: 0.,
                backdrop: 0.,
            },
            elements: nodes
                .iter()
                .enumerate()
                .map(|(i, title)| StageElement::Card {
                    id: format!("node-{i}"),
                    at: [315. + i as f32 * 430., 515., 0.],
                    size: [310., 108.],
                    title: (*title).into(),
                    status: vec![],
                    tone: Tone::Plain,
                    mark: Mark::Check,
                })
                .chain((0..3).map(|i| StageElement::Beam {
                    id: format!("link-{i}"),
                    from: format!("node-{i}"),
                    to: format!("node-{}", i + 1),
                    bend: 0.,
                    tone: Tone::Muted,
                }))
                .collect(),
        },
    )?;
    for (i, note) in notes.iter().enumerate() {
        // Existing cards stay still. Only the new causal path draws on.
        reveal(&mut p, &diagram, &format!("node-{i}.opacity"), i, 4, 0.4);
        let actor = prose(
            &mut p,
            &format!("note-{i}"),
            note,
            [160. + i as f32 * 430., 640.],
            310.,
            27.,
        )?;
        reveal(&mut p, &actor, "opacity", i, 4, 0.16);
        if i > 0 {
            reveal(&mut p, &diagram, &format!("link-{}.draw", i - 1), i, 4, 0.6);
        }
    }
    let actor = prose(
        &mut p,
        "takeaway",
        &format!("**{takeaway}**"),
        [145., 875.],
        1600.,
        30.,
    )?;
    reveal(&mut p, &actor, "opacity", 3, 4, 0.16);
    Ok(SlidePlan {
        title: title.into(),
        plan: p.finish()?,
    })
}

pub fn build_deck() -> Result<DeckPlan> {
    let slides = vec![
        columns(
            1,
            "THE PROPOSAL / TWO-WEEK PILOT",
            "Spend tokens once. Keep the protection.",
            "Use agents to investigate real user problems. Keep their findings as executable checks.",
            &[
                "**Replay real failures**\n\nTurn high-impact reports into small regressions that fail before a repair and pass after it.",
                "**Stress critical transitions**\n\nExercise restarts, retries, upgrades, and reconnects—not only features in isolation.",
                "**Check the experience**\n\nCompare actual interfaces and run docs-only newcomer journeys against shipped builds.",
            ],
            "Approve a bounded pilot—not a new testing platform.",
            "Team proposal / 02 October 2026 / recommendations, not measured savings",
        )?,
        columns(
            2,
            "WHY THIS MATTERS",
            "Users experience journeys, not modules.",
            "A feature can work in isolation while the product fails at the moment someone needs it.",
            &[
                "**Start working**\n\nInstall → configure → connect.\n\nIf setup fails, the user never reaches the model.",
                "**Keep working**\n\nSubmit → run tools → reconnect.\n\nIf recovery fails, a working session becomes a support incident.",
                "**Return to work**\n\nUpgrade → find session → reopen.\n\nData that exists but cannot be found still feels lost.",
            ],
            "Prioritize interruptions to real workflows over the number of tests or agent runs.",
            "Reasoning from reported journeys / not a claim that existing tests are absent",
        )?,
        columns(
            3,
            "EVIDENCE / REPORTED, NOT REPRODUCED HERE",
            "Three reports. Three different blind spots.",
            "The earlier review sampled 60 recent open and 40 recently closed issues—not the whole backlog.",
            &[
                "**Migration visibility**\n\n#52844: old sessions reportedly survive in storage but disappear from the TUI list.\n\nCheck migration **and** find/reopen.",
                "**Persisted desktop state**\n\n#52736: stale workspace scopes reportedly trigger a launch-time allocation loop.\n\nTest used profiles, not only fresh ones.",
                "**Strict MCP peers**\n\n#52758: a strict server reportedly rejects initialization before authentication.\n\nTest protocol behavior, not only permissive mocks.",
            ],
            "44 of the 60 open reports had no labels. Triage the behavior, not just the metadata.",
            "GitHub #52844 / #52736 / #52758 / issue sample is directional, not a prevalence estimate",
        )?,
        flow(
            (4, "THE COMPOUNDING LOOP"),
            "Do not rent the same insight twice.",
            "A recurring review spends again. A retained regression makes the next check ordinary test work.",
            &["Report", "Failing proof", "Scoped repair", "Routine replay"],
            &[
                "One user outcome. Exact version and environment.",
                "Small fixture. Failure tied to the intended contract.",
                "Same proof passes. Working controls stay green.",
                "Issue and test linked. Cheap replay on relevant changes.",
            ],
            "The output of agent work is durable evidence—not another opinion.",
            "Expected return on effort / model spend and reviewer time must be measured",
        )?,
        columns(
            5,
            "PRIORITY 1 / ISSUE-TO-REGRESSION",
            "Require evidence on both sides of a repair.",
            "Keep the acceptance criterion stable. A test rewritten to fit the patch is weak evidence.",
            &[
                "**Before**\n\nPin the base revision.\n\nRun the intended positive assertion against it and retain the failure.\n\nDistinguish a product failure from fixture setup errors.",
                "**After**\n\nRun the same proof against the candidate.\n\nKeep healthy controls and original user journey checks.\n\nChallenge coverage at the actual failing boundary.",
                "**Retain**\n\nSave a replay command, safe fixture, assertion, and evidence limits.\n\nLink issue → regression → release.\n\nUser-confirmed recovery remains separate.",
            ],
            "One repair owns one behavioral contract. Unrelated cleanup stays out.",
            "Build on the existing managed-service proof campaign / see presenter notes for evidence limits",
        )?,
        flow(
            (6, "WORKED EXAMPLE / PROPOSED REGRESSION"),
            "A migration is not done when rows copy.",
            "For #52844, the user contract is: an existing session can still be found and reopened after upgrade.",
            &["Old fixture", "Upgrade", "Find session", "Reopen"],
            &[
                "Synthetic legacy data: root, subdirectory, and missing-path cases.",
                "Run the actual migration in a disposable profile.",
                "Query through the same project and subpath view the TUI uses.",
                "Open the session. Verify content and location are retained.",
            ],
            "A CLI list or row-count assertion alone would miss the reported TUI failure.",
            "Report #52844 / proposed acceptance test / not a repair or reproduction completed by this deck",
        )?,
        columns(
            7,
            "PRIORITY 2 / TRANSITION STRESS",
            "Break the transition. Preserve the promise.",
            "Use real runtime paths with controlled faults. Record the seed and minimize every new failure.",
            &[
                "**Owner / reconnect**\n\nRestart at bootstrap.\n\nRace two starters.\n\nDelay readiness or cleanup.\n\nAssert valid ownership and useful, bounded recovery.",
                "**Prompt / retry**\n\nDrop an acknowledgement.\n\nRetry the same submission ID.\n\nInterrupt during a tool call.\n\nAssert the intended delivery and recovery contract.",
                "**Version / state**\n\nPair old and new clients.\n\nLoad synthetic old data.\n\nReopen after upgrade.\n\nAssert compatibility or an actionable error.",
            ],
            "Order faults with barriers where possible. Random run count is not coverage.",
            "Proposed schedules / disposable profiles, owned processes, and bounded deadlines only",
        )?,
        columns(
            8,
            "THE EVIDENCE TRAP",
            "A green check can still miss the bug.",
            "A useful proof states what it exercises—and what it bypasses.",
            &[
                "**Pinned server ≠ election**\n\nA Drive scenario using an explicit server can test UI recovery without exercising managed discovery.\n\nDo not claim both from one run.",
                "**Simulation ≠ full boot**\n\nExisting campaign records include 100 passing seeded process-fault cases.\n\nThey used controlled health responses, not full server boot or the TUI.",
                "**Local ≠ native matrix**\n\nLoopback success on one machine cannot establish Windows/WSL networking or packaged desktop behavior.\n\nKeep those gates explicit.",
            ],
            "Extend existing proofs to missing boundaries. Do not rebuild what already works.",
            "Recorded campaign evidence, not rerun for this presentation / no universal reliability claim",
        )?,
        columns(
            9,
            "PRIORITY 3 / PRODUCT POLISH",
            "Make “shiny” a repeatable check.",
            "Functional assertions tell us what happened. Matched captures help us see what the user experienced.",
            &[
                "**Matched UI comparisons**\n\nSame fixture, viewport, theme, and state. Before and after.\n\nCover focus, scrolling, long content, pending dialogs, and narrow terminals.\n\nUse production components—not visual copies.",
                "**Docs-only newcomer runs**\n\nClean profile. Packaged build. Public docs only.\n\nTry provider setup, MCP connection, and reopening a session.\n\nRecord every blocker and undocumented workaround.",
            ],
            "Agents flag friction. Humans own taste. Compare on UI changes; run newcomer checks weekly.",
            "Reuse TUI Drive and app visual-stability tooling / proposed cadence, not a new blanket CI gate",
        )?,
        flow(
            (10, "ISSUE TRACKING / EVIDENCE STATES"),
            "Closed is not the same as recovered.",
            "Track the strongest evidence we actually have. Never make a status update stand in for proof.",
            &["Reported", "Reproduced", "Repaired", "Released"],
            &[
                "Behavior, version, environment, and user impact.",
                "Replayable failure. Or explicitly unconfirmed.",
                "Candidate passes the unchanged proof and controls.",
                "Fix shipped. User-confirmed recovery tracked separately.",
            ],
            "GitHub: public problem. Organizer: private campaign. Repository: executable proof.",
            "Link records instead of duplicating backlogs / no automatic closing or outbound replies",
        )?,
        columns(
            11,
            "AGENT OPERATING MODEL",
            "Parallelize problems—not opinions.",
            "Clear roles and a stable contract give reviewers something concrete to challenge.",
            &[
                "**Reproducer**\n\nOne claim and a small replay.\n\nCapture base revision, fault boundary, intended assertion, and limitations.\n\nStop when evidence is insufficient.",
                "**Fixer**\n\nRepair one contract.\n\nDo not move the acceptance criterion.\n\nDo not expand into unrelated refactors merely because the agent can.",
                "**Verifier**\n\nRerun the unchanged proof.\n\nChallenge healthy controls and the original journey.\n\nCheck whether the fixture made success inevitable.",
            ],
            "Bound tokens, time, and attempts. Escalate repeated failures; do not loop forever.",
            "A role separation is a review aid, not a guarantee of agent independence or correctness",
        )?,
        columns(
            12,
            "TWO DIFFERENT EVALUATION LOOPS",
            "Use models where model behavior matters.",
            "Scripted dependencies make runtime tests stable. They cannot tell us whether an agent still solves tasks.",
            &[
                "**Deterministic replay**\n\nProtocol, migration, state, lifecycle, and UI contracts.\n\nControlled model responses.\n\nRoutine reruns need no model calls; compute and maintenance still cost.",
                "**Small real-model suite**\n\nCoding outcomes, steering, tool use, and compaction.\n\nPinned tasks and budgets; repeated trials where results vary.\n\nScore completed user outcomes—not just matching transcripts.",
            ],
            "Keep the suites separate so a model variance problem does not masquerade as a runtime regression.",
            "Agent evaluation guidance: anthropic.com/engineering/demystifying-evals-for-ai-agents",
        )?,
        columns(
            13,
            "RANKING / WHAT NOT TO DO",
            "Concentrate effort before expanding scope.",
            "These are expected-return judgments—not measured OpenCode savings.",
            &[
                "**Start now**\n\nIssue-to-regression.\n\nLifecycle and upgrade boundaries.\n\nMatched UI + newcomer checks.\n\nClosest to concrete user pain and durable protection.",
                "**Add selectively**\n\nBacklog evidence audits.\n\nA small real-model suite.\n\nMutation tests of critical assertions: does a deliberate defect make the test fail?",
                "**Do not default to**\n\nRecurring whole-repo opinion sweeps.\n\nSeveral agents re-reviewing the same diff.\n\nA new generic orchestration platform before the first useful proof.",
            ],
            "The quality bar is evidence retained per hour and per dollar—not tokens consumed.",
            "Mutation guidance: Practical Mutation Testing at Scale / Google Research",
        )?,
        columns(
            14,
            "PILOT / TWO WEEKS",
            "Five clusters. Three durable proofs.",
            "One accountable maintainer. A pre-agreed spend ceiling. Existing tools first.",
            &[
                "**Week one: establish**\n\nRank five clusters by user impact, recurrence evidence, and reproducibility.\n\nChoose three proof targets.\n\nCapture intended baseline failures and controls.",
                "**Week two: validate**\n\nVerify available candidates or route focused repair work through normal review.\n\nRun one newcomer journey and matched UI checks.\n\nRetain failures even when no repair is ready.",
                "**Daily: short brief**\n\nWhat failed?\n\nWhat is now protected?\n\nWhat needs a human decision?\n\nLink evidence; log spend and reviewer time.",
            ],
            "Three proof targets are a goal—not a promise that three fixes ship in two weeks.",
            "Suggested first clusters: migration visibility / persisted desktop startup / strict MCP initialization",
        )?,
        columns(
            15,
            "MEASUREMENT / CONTINUE OR STOP",
            "Prove the loop pays for itself.",
            "Report quality and cost together. A cheaper run is not a win if it loses the user contract.",
            &[
                "**User value**\n\nWhich blocked workflows have verified repairs?\n\nWhat coverage was retained?\n\nDo later related changes trigger the proof?\n\nRecurrence is a longer-term follow-up.",
                "**Operating cost**\n\nModel spend per durable proof.\n\nReviewer minutes per accepted result.\n\nFlaky runs and maintenance.\n\nBlocked reproductions and abandoned attempts.",
                "**Decision gate**\n\nContinue if targeted proofs expose intended failures and remain useful, replayable, and affordable.\n\nNarrow or stop if outputs are opinions, fixtures are brittle, or review effort outweighs value.",
            ],
            "Approve the pilot and its owner. Set the ceiling together; decide on evidence at day 14.",
            "No invented ROI forecast / no activity-count success metric",
        )?,
        columns(
            16,
            "APPENDIX / USE WHAT ALREADY EXISTS",
            "The starting point is not zero.",
            "Inspected assets establish reuse opportunities—not proof that every required scenario is covered.",
            &[
                "**Service contracts**\n\nClient service tests.\n\nExisting managed-service proof campaign and process-fault fixtures.\n\nExtend the missing fault boundaries and native matrix.",
                "**User-interface proof**\n\nDrive for real TUI behavior.\n\nApp visual-stability tooling and timeline scenario matrices.\n\nAdd the specific reported journey before adding infrastructure.",
                "**Provider behavior**\n\nAI transport recording fixtures.\n\nDeterministic playback for protocol regressions.\n\nSeparate real-model tasks when outcomes—not transport—are the question.",
            ],
            "Do a narrow coverage audit for the chosen contract. A file inventory is not a coverage guarantee.",
            "Local source inspection / packages/client/test, packages/app/e2e, packages/ai/test/fixtures",
        )?,
        columns(
            17,
            "APPENDIX / SOURCES AND LIMITS",
            "Strong proposal. Honest evidence limits.",
            "Presenter context and clickable sources are included in the companion notes.",
            &[
                "**User reports**\n\nGitHub issues #52844, #52736, #52758.\n\nEarlier bounded sample: 60 open, 40 recently closed.\n\nReports were read, not independently reproduced for this deck.",
                "**Existing proof work**\n\nManaged-service campaign records and source inventory.\n\nPR #50825 documents reconnect preservation and controls.\n\nThat PR was open when checked; no claim of a released fix.",
                "**Testing practice**\n\nAnthropic: agent evaluations.\n\nfast-check: replayable model-based testing.\n\nGoogle Research: targeted mutation testing.\n\nThese inform the method—not a forecast of savings.",
            ],
            "The proposal asks us to test the economics. It does not pretend we already know them.",
            "Research snapshot / 02 October 2026 / full notes: scenes/opencode-quality/PRESENTER.md",
        )?,
    ];
    let deck = DeckPlan {
        version: 1,
        id: "opencode-quality".into(),
        slides,
    };
    deck.validate()?;
    Ok(deck)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deck_is_valid_and_uses_only_native_continuous_recipes() {
        let deck = build_deck().unwrap();
        assert_eq!(deck.slides.len(), 17);
        for slide in &deck.slides {
            assert!(slide.plan.state_channels.is_empty());
            assert!(slide.plan.media.is_empty());
            assert!(
                slide
                    .plan
                    .actors
                    .iter()
                    .all(|a| matches!(a.recipe.as_str(), TYPESET | RICH_TEXT | STAGE_RECIPE))
            );
            assert!(!slide.plan.presentation_steps.is_empty());
        }
    }

    #[test]
    fn retained_text_and_cards_do_not_move_or_get_replaced() {
        let deck = build_deck().unwrap();
        for slide in &deck.slides {
            for channel in &slide.plan.continuous_channels {
                assert!(
                    channel.property == "opacity"
                        || channel.property.ends_with(".opacity")
                        || channel.property.ends_with(".draw")
                );
                assert_ne!(channel.actor_id, "title");
                assert_ne!(channel.actor_id, "subtitle");
            }
        }
    }

    #[test]
    fn emission_is_deterministic() {
        let first = build_deck().unwrap();
        let second = build_deck().unwrap();
        assert_eq!(format!("{first:?}"), format!("{second:?}"));
    }
}

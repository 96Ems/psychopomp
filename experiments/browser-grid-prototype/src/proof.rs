//! Experiment-local case inventory. All hosts consume the emitted manifest;
//! names and required cases are not inferred from whatever PNGs happen to exist.
use serde::Serialize;

pub const INTERRUPTIONS: [(u64, &str); 16] = [
    (0,"next"),(50,""),(90,"next"),(149,""),(150,"previous"),(151,""),
    (210,"last"),(225,""),(280,"previous"),(300,""),(370,"first"),
    (390,""),(420,"last"),(450,""),(800,""),(2200,""),
];

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
pub struct Case {
    pub id: String,
    pub group: &'static str,
    pub scene: &'static str,
    pub theme: &'static str,
    pub native: String,
    pub sample: Sample,
}

#[derive(Serialize)]
#[serde(tag="kind",rename_all="camelCase")]
pub enum Sample {
    Authored { seconds: f64 },
    Interactive { millis: u64, action: &'static str },
}

pub fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for scene in ["growing-grid","grid-isomorphism","daemon-merge","daemon-isometric"] {
        let diagram = scene.starts_with("daemon-");
        let times: &[f64] = if diagram {&[0.,3.12,3.2,6.2,8.,9.16,9.2,11.]} else {&[0.,3.2,9.2,17.,21.2,23.]};
        for &seconds in times {
            let (id, group) = if diagram {(format!("{scene}-{seconds}"),"diagram")}
                else if scene=="growing-grid" {(format!("authored-{seconds}"),"grid")}
                else {(format!("reference-{scene}-{seconds}"),"reference")};
            let time = if diagram {format!("{seconds}")}else{format!("{seconds:.1}")};
            cases.push(Case {id,group,scene,theme:"original",native:format!("native-{scene}-{time}.png"),sample:Sample::Authored {seconds}});
        }
        if scene=="grid-isomorphism" {continue;}
        for (millis,action) in INTERRUPTIONS {
            let id = if diagram {format!("{scene}-interrupt-{millis}")}else{format!("interrupt-{millis}")};
            cases.push(Case {native:format!("native-{id}.png"),id,group:if diagram {"diagram"}else{"grid"},scene,theme:"original",sample:Sample::Interactive {millis,action}});
        }
    }
    for (id,scene,theme,seconds) in [
        ("regroup-left","grid-isomorphism","original",3.2),
        ("regroup-reverse","grid-isomorphism","original",9.2),
        ("black-volume","growing-grid","black",17.),
        ("evergreen-cutaway","growing-grid","evergreen",21.2),
        ("tokyo-growth","growing-grid","tokyo-night",3.2),
    ] {
        cases.push(Case {id:id.into(),group:"grid",scene,theme,native:format!("native-{id}.png"),sample:Sample::Authored {seconds}});
    }
    cases
}

#[cfg(test)]
mod tests {
    #[test]
    fn inventory_contains_all_cases_with_unique_names() {
        let cases = super::cases();
        assert_eq!(cases.iter().filter(|c|c.group=="grid").count(),27);
        assert_eq!(cases.iter().filter(|c|c.group=="diagram").count(),48);
        let names = cases.iter().map(|c|&c.id).collect::<std::collections::HashSet<_>>();
        assert_eq!(names.len(),cases.len());
        for name in ["regroup-left","regroup-reverse","black-volume","evergreen-cutaway","tokyo-growth"] {
            assert!(names.iter().any(|id|id.as_str()==name));
        }
    }
}

//! A finite product catalog and its authored arrangements. The catalog is immutable:
//! changing arrangement never changes the identity of a tuple.
use std::collections::HashSet;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

pub const GRID_RECIPE: &str = "keyed-grid";
pub const MAX_GRID_CELLS: usize = 256;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridRecipePlan {
    pub axes: [GridAxisPlan; 3],
    /// Optional human-readable representations; tuple identity remains in the catalog.
    #[serde(default)]
    pub labels: Vec<GridCellLabelPlan>,
    pub initial: GridSnapshotPlan,
    #[serde(default)]
    pub events: Vec<GridEventPlan>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridCellLabelPlan {
    pub indices: [usize; 3],
    pub primary: String,
    #[serde(default)]
    pub secondary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridAxisPlan {
    pub name: String,
    /// Unique, short value names. Their order is fixed for the lifetime of the actor.
    pub values: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridSnapshotPlan {
    /// Visible prefixes of the three axes. A row/table is a slice of the full product.
    pub visible: [usize; 3],
    pub arrangement: GridArrangement,
    #[serde(default)]
    pub focus_slice: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GridArrangement {
    /// Straight-on orthographic view. Depth still exists, but projects behind the front grid.
    Table,
    /// The same connected lattice viewed obliquely from above.
    Layers,
    /// ((a, b), c): one A × B table per C value.
    LeftAssociated,
    /// (a, (b, c)): one B × C table per A value.
    RightAssociated,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridEventPlan {
    pub at_nanos: u64,
    pub snapshot: GridSnapshotPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridCell {
    pub indices: [usize; 3],
    pub values: [String; 3],
}

impl GridCell {
    pub fn key(&self) -> String {
        let [a, b, c] = self.indices;
        format!("cell.{a}.{b}.{c}")
    }
}

impl GridRecipePlan {
    pub fn dimensions(&self) -> [usize; 3] {
        self.axes.each_ref().map(|axis| axis.values.len())
    }

    /// Bounded, deterministic enumeration, independent of glyphs and layout.
    pub fn cells(&self) -> Result<Vec<GridCell>> {
        self.validate_catalog()?;
        let [a, b, c] = self.dimensions();
        Ok((0..c)
            .flat_map(|z| (0..b).flat_map(move |y| (0..a).map(move |x| [x, y, z])))
            .map(|indices| GridCell {
                values: std::array::from_fn(|axis| self.axes[axis].values[indices[axis]].clone()),
                indices,
            })
            .collect())
    }

    pub fn validate(&self, duration: u64) -> Result<()> {
        self.validate_catalog()?;
        self.validate_snapshot(&self.initial)?;
        let mut previous = 0;
        for event in &self.events {
            if event.at_nanos < previous || event.at_nanos > duration {
                bail!("grid events must be ordered and inside the scene duration");
            }
            self.validate_snapshot(&event.snapshot)?;
            previous = event.at_nanos;
        }
        Ok(())
    }

    fn validate_catalog(&self) -> Result<()> {
        let mut names = HashSet::new();
        let mut count = 1_usize;
        for axis in &self.axes {
            if axis.name.trim().is_empty() || !names.insert(&axis.name) {
                bail!("grid axis names must be nonempty and unique");
            }
            let mut values = HashSet::new();
            for value in &axis.values {
                if value.trim().is_empty() || value.chars().count() > 8 || !values.insert(value) {
                    bail!("grid values must be unique, nonempty names of at most eight characters");
                }
            }
            count = count.saturating_mul(axis.values.len());
        }
        if count == 0 || count > MAX_GRID_CELLS {
            bail!("grid catalog must contain 1..={MAX_GRID_CELLS} cells");
        }
        let mut labels = HashSet::new();
        for label in &self.labels {
            if label
                .indices
                .iter()
                .zip(self.dimensions())
                .any(|(&i, n)| i >= n)
                || !labels.insert(label.indices)
                || label.primary.trim().is_empty()
            {
                bail!("grid labels need a unique catalog tuple and nonempty primary text");
            }
        }
        Ok(())
    }

    fn validate_snapshot(&self, snapshot: &GridSnapshotPlan) -> Result<()> {
        if snapshot
            .visible
            .iter()
            .zip(self.dimensions())
            .any(|(&v, n)| v == 0 || v > n)
        {
            bail!("grid visible prefixes must be nonempty and inside the catalog");
        }
        if let Some(slice) = snapshot.focus_slice
            && (snapshot.arrangement != GridArrangement::Layers || slice >= snapshot.visible[2])
        {
            bail!("grid focus must select a visible slice of the layers arrangement");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe() -> GridRecipePlan {
        GridRecipePlan {
            axes: std::array::from_fn(|i| GridAxisPlan {
                name: ["A", "B", "C"][i].into(),
                values: (0..[3, 2, 4][i]).map(|v| v.to_string()).collect(),
            }),
            labels: vec![],
            initial: GridSnapshotPlan {
                visible: [3, 1, 1],
                arrangement: GridArrangement::Table,
                focus_slice: None,
            },
            events: vec![],
        }
    }

    #[test]
    fn product_has_unique_stable_tuples_and_reassociation_round_trips() {
        let mut recipe = recipe();
        let cells = recipe.cells().unwrap();
        assert_eq!(cells.len(), 24);
        assert_eq!(
            cells
                .iter()
                .map(GridCell::key)
                .collect::<HashSet<_>>()
                .len(),
            24
        );
        for cell in &cells {
            let [a, b, c] = cell.values.clone();
            let left = ((a, b), c);
            let ((a, b), c) = left.clone();
            let right = (a, (b, c));
            let (a, (b, c)) = right;
            assert_eq!(((a, b), c), left);
        }
        recipe.initial.arrangement = GridArrangement::RightAssociated;
        recipe.initial.visible = [3, 2, 4];
        assert_eq!(recipe.cells().unwrap(), cells);
    }

    #[test]
    fn rejects_ambiguous_catalogs_and_invalid_snapshots() {
        let mut r = recipe();
        r.axes[0].values.push("0".into());
        assert!(r.cells().is_err());
        let mut r = recipe();
        r.axes[1].values.clear();
        assert!(r.cells().is_err());
        let mut r = recipe();
        r.initial.visible = [4, 1, 1];
        assert!(r.validate(10).is_err());
        r.initial.visible = [3, 2, 4];
        assert!(r.validate(10).is_ok());
        r.initial.arrangement = GridArrangement::Layers;
        r.initial.focus_slice = Some(4);
        assert!(r.validate(10).is_err());
        r.initial.focus_slice = Some(1);
        assert!(r.validate(10).is_ok());
        r.events.push(GridEventPlan {
            at_nanos: 11,
            snapshot: r.initial.clone(),
        });
        assert!(r.validate(10).is_err());
    }

    #[test]
    fn cell_labels_do_not_change_identity_and_must_address_unique_tuples() {
        let mut recipe = recipe();
        let cells = recipe.cells().unwrap();
        recipe.labels.push(GridCellLabelPlan {
            indices: [0, 0, 0],
            primary: "♖".into(),
            secondary: "White · I".into(),
        });
        assert_eq!(recipe.cells().unwrap(), cells);
        recipe.labels.push(recipe.labels[0].clone());
        assert!(recipe.validate(10).is_err());
        recipe.labels.pop();
        recipe.labels[0].indices = [3, 0, 0];
        assert!(recipe.validate(10).is_err());
    }
}

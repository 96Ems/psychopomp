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
    /// Omitted preserves the original cube presentation and serialized plans.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<GridStylePlan>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GridStylePlan {
    pub fill: GridFillPlan,
    pub rules: GridRules,
    pub line_width: f32,
    pub line_opacity: f32,
    pub table: Option<GridTableLayout>,
}

impl Default for GridStylePlan {
    fn default() -> Self {
        Self {
            fill: GridFillPlan::Checkerboard,
            rules: GridRules::Grid,
            line_width: 1.7,
            line_opacity: 1.,
            table: None,
        }
    }
}

impl GridStylePlan {
    pub fn plain_table(column_widths: Vec<f32>) -> Self {
        Self {
            fill: GridFillPlan::None,
            rules: GridRules::Grid,
            line_width: 1.,
            line_opacity: 0.4,
            table: Some(GridTableLayout {
                column_widths,
                row_height: 84.,
                font_size: 28.,
                padding: 24.,
                alignments: vec![],
                headers: vec![],
            }),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum GridFillPlan {
    #[default]
    Checkerboard,
    /// Background-matching opaque material, not transparent wireframe.
    None,
    Uniform {
        color: [u8; 3],
    },
    Banded {
        color: [u8; 3],
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GridRules {
    #[default]
    Grid,
    Rows,
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GridAlignment {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GridTableLayout {
    pub column_widths: Vec<f32>,
    pub row_height: f32,
    pub font_size: f32,
    /// Horizontal inset in unscaled scene pixels.
    pub padding: f32,
    #[serde(default)]
    pub alignments: Vec<GridAlignment>,
    /// Optional display headings, independent of immutable column identity.
    #[serde(default)]
    pub headers: Vec<String>,
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
        if let Some(style) = &self.style {
            if !style.line_width.is_finite()
                || !(0.5..=4.).contains(&style.line_width)
                || !style.line_opacity.is_finite()
                || !(0.0..=1.).contains(&style.line_opacity)
            {
                bail!("grid line width must be in [0.5,4] and opacity in [0,1]");
            }
            if let Some(table) = &style.table {
                let dims = self.dimensions();
                if dims[2] != 1
                    || std::iter::once(&self.initial)
                        .chain(self.events.iter().map(|e| &e.snapshot))
                        .any(|s| {
                            matches!(
                                s.arrangement,
                                GridArrangement::LeftAssociated | GridArrangement::RightAssociated
                            )
                        })
                {
                    bail!(
                        "table layout supports one depth layer in Table or Layers arrangement; regrouping remains a cube-layout operation"
                    );
                }
                if table.column_widths.len() != dims[0]
                    || table
                        .column_widths
                        .iter()
                        .any(|w| !w.is_finite() || !(64.0..=900.).contains(w))
                    || !table.row_height.is_finite()
                    || !(40.0..=240.).contains(&table.row_height)
                    || !table.font_size.is_finite()
                    || !(12.0..=64.).contains(&table.font_size)
                    || !table.padding.is_finite()
                    || table.padding < 0.
                    || table.column_widths.iter().any(|w| 2. * table.padding >= *w)
                    || table.row_height
                        < table.font_size
                            * if self.labels.iter().any(|l| !l.secondary.is_empty()) {
                                2.5
                            } else {
                                1.5
                            }
                    || (!table.alignments.is_empty() && table.alignments.len() != dims[0])
                    || (!table.headers.is_empty() && table.headers.len() != dims[0])
                    || table
                        .headers
                        .iter()
                        .any(|s| s.trim().is_empty() || s.contains(['\n', '\r']))
                {
                    bail!(
                        "invalid table widths, row height, typography, padding, alignments, or display headings"
                    );
                }
                if self
                    .labels
                    .iter()
                    .any(|l| l.primary.contains(['\n', '\r']) || l.secondary.contains(['\n', '\r']))
                {
                    bail!("table cells require single-line primary and secondary text");
                }
            }
        }
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
            style: None,
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

    #[test]
    fn presentation_is_optional_and_never_changes_cell_identity() {
        let mut recipe = recipe();
        let cells = recipe.cells().unwrap();
        assert!(
            serde_json::to_value(&recipe)
                .unwrap()
                .get("style")
                .is_none()
        );
        for fill in [
            GridFillPlan::None,
            GridFillPlan::Uniform {
                color: [24, 28, 39],
            },
            GridFillPlan::Banded {
                color: [30, 35, 45],
            },
        ] {
            recipe.style = Some(GridStylePlan {
                fill,
                ..Default::default()
            });
            recipe.validate(10).unwrap();
            assert_eq!(recipe.cells().unwrap(), cells);
        }
    }

    #[test]
    fn plain_table_validates_its_real_geometry_not_a_square_cell_assumption() {
        let mut r = recipe();
        r.axes[2].values.truncate(1);
        r.initial.visible = [3, 2, 1];
        r.style = Some(GridStylePlan::plain_table(vec![400., 280., 200.]));
        r.validate(10).unwrap();
        let mut invalid = r.clone();
        invalid
            .style
            .as_mut()
            .unwrap()
            .table
            .as_mut()
            .unwrap()
            .column_widths
            .pop();
        assert!(invalid.validate(10).is_err());
        let mut invalid = r.clone();
        invalid
            .style
            .as_mut()
            .unwrap()
            .table
            .as_mut()
            .unwrap()
            .padding = 110.;
        assert!(invalid.validate(10).is_err());
        let mut invalid = r.clone();
        invalid
            .style
            .as_mut()
            .unwrap()
            .table
            .as_mut()
            .unwrap()
            .alignments = vec![GridAlignment::Right];
        assert!(invalid.validate(10).is_err());
        let mut invalid = r.clone();
        invalid.initial.arrangement = GridArrangement::RightAssociated;
        assert!(invalid.validate(10).is_err());
        let mut invalid = r.clone();
        invalid.style.as_mut().unwrap().line_width = f32::NAN;
        assert!(invalid.validate(10).is_err());
        r.initial.arrangement = GridArrangement::Layers;
        r.validate(10).unwrap();
    }
}

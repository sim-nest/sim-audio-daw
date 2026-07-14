//! Deterministic cookbook builders for audio-graph core recipes.

use crate::{Patch, PatchNode};

use sim_kernel::Expr;

/// Build the modeled copy-node patch descriptor used by the cookbook recipe.
pub fn copy_node_demo() -> Expr {
    Patch {
        nodes: vec![PatchNode {
            id: "copy".to_owned(),
            in_channels: 1,
            out_channels: 1,
        }],
        cables: Vec::new(),
    }
    .to_expr()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Patch;

    #[test]
    fn copy_node_demo_round_trips_as_patch() {
        let expr = copy_node_demo();
        let patch = Patch::from_expr(&expr).expect("copy node patch decodes");
        assert_eq!(patch.nodes.len(), 1);
        assert_eq!(patch.nodes[0].id, "copy");
    }
}

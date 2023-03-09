//! Module to parse the output of `cargo-geiger`
//!
//! This module relies on the using the flags `--output-format Json --quiet`.
//! The output can still be quite noicy, so sometimes running `2>/dev/null` is
//! required (pipe `stderr` to the black hole at the center of the Linuxverse).
//!
//! The output of `cargo-geiger` is on the form (some fields omitted)
//! ```json
//! {
//!     "packages": [
//!         {
//!             "package": {
//!                 "id": {
//!                     "name": "libc",
//!                     "version": "0.2.139",
//!                     // Etc.
//!                 },
//!                 "unsafety": {
//!                     "used": {
//!                         "functions": {"safe":0,"unsafe_":0},
//!                         "exprs": {"safe":253,"unsafe_":12},
//!                         "item_impls": {"safe":11,"unsafe_":0},
//!                         "item_traits":{"safe":0,"unsafe_":0},
//!                         "methods":{"safe":5,"unsafe_":2}
//!                     },
//!                     "unused": {
//!                         "functions": {"safe":6,"unsafe_":24},
//!                         "exprs": {"safe":3934,"unsafe_":432},
//!                         "item_impls": {"safe":103,"unsafe_":2},
//!                         "item_traits": {"safe":0,"unsafe_":0},
//!                         "methods":{"safe":46,"unsafe_":43}},
//!                     },
//!                     "forbids_unsafe":false
//!                 }
//!             }
//!         }
//!     ]
//! }
//! ```
//!
//! The target of this module is to make it easy to extract the data in the schema;
//! In general this is achieved by a `total` method that allows for aggregating
//! for example used+unused, and at a lower level safe+unsafe_.

use std::{collections::HashMap, ops::Add};

use serde::Deserialize;

/// Calculates a percentage, rounded to two
/// decimal points
///
/// This function will handle `0 / 0` to be equal to `0.0` (all code is safe,
/// there is no code).
pub(crate) fn two_digit_percentage(part: u32, total: u32) -> f64 {
    let res = f64::from(part) / f64::from(total);
    if res.is_finite() {
        // We only really care about at most two decimal points
        (res * 10000.0).round() / 100.0
    } else {
        0.0
    }
}

/// The full output of `cargo-geiger`
#[derive(Debug, Clone, Deserialize)]
pub struct GeigerOutput {
    packages: Vec<GeigerPackageOutput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeigerPackageOutput {
    unsafety: GeigerUnsafety,
    id: HashMap<String, String>,
}

/// The output of `cargo-geiger` for one package/dependency
///
/// Corresponds to the object named "unsafety" in a `cargo-geiger` output.
/// `used` and `unused` refers to if the code is used by the package used
/// to provide the Geiger data. A package may have a high unsafe usage, but
/// nothing is used by the analyzed package.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct GeigerUnsafety {
    pub used: GeigerTargets,
    pub unused: GeigerTargets,
    pub forbids_unsafe: bool,
}

impl GeigerUnsafety {
    /// Retrieves the total geiger count for all targets
    pub fn total(&self) -> GeigerTargets {
        GeigerTargets {
            functions: self.used.functions + self.unused.functions,
            exprs: self.used.exprs + self.unused.exprs,
            item_impls: self.used.item_impls + self.unused.item_impls,
            item_traits: self.used.item_traits + self.unused.item_traits,
            methods: self.used.methods + self.unused.methods,
        }
    }

    pub fn used_safe(&self) -> u32 {
        self.used.total_safe()
    }

    pub fn used_unsafe(&self) -> u32 {
        self.used.total_unsafe()
    }

    pub fn unused_safe(&self) -> u32 {
        self.unused.total_safe()
    }

    pub fn unused_unsafe(&self) -> u32 {
        self.unused.total_unsafe()
    }

    pub fn total_safe(&self) -> u32 {
        self.used_safe() + self.unused_safe()
    }

    pub fn total_unsafe(&self) -> u32 {
        self.used_unsafe() + self.unused_unsafe()
    }

    /// Calculates the percentage of the package to be unsafe, to two decimal
    /// points
    pub fn percentage_unsafe(&self) -> f64 {
        two_digit_percentage(
            self.total_unsafe(),
            self.total_safe() + self.total_unsafe(),
        )
    }
}

/// All different targets in Rust code that `cargo-geiger` counts
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct GeigerTargets {
    pub functions: GeigerCount,
    pub exprs: GeigerCount,
    pub item_impls: GeigerCount,
    pub item_traits: GeigerCount,
    pub methods: GeigerCount,
}

impl GeigerTargets {
    /// Aggregates all [`GeigerCounts`] for all targets, returning one with total
    /// safe and total unsafe for all targets
    pub fn total(&self) -> GeigerCount {
        self.functions
            + self.exprs
            + self.item_impls
            + self.item_traits
            + self.methods
    }

    pub fn total_safe(&self) -> u32 {
        self.functions.safe
            + self.exprs.safe
            + self.item_impls.safe
            + self.item_traits.safe
            + self.methods.safe
    }

    pub fn total_unsafe(&self) -> u32 {
        self.functions.unsafe_
            + self.exprs.unsafe_
            + self.item_impls.unsafe_
            + self.item_traits.unsafe_
            + self.methods.unsafe_
    }
}

impl Add<GeigerTargets> for GeigerTargets {
    type Output = GeigerTargets;

    fn add(self, rhs: GeigerTargets) -> Self::Output {
        GeigerTargets {
            functions: self.functions + rhs.functions,
            exprs: self.exprs + rhs.exprs,
            item_impls: self.item_impls + rhs.item_impls,
            item_traits: self.item_traits + rhs.item_traits,
            methods: self.methods + rhs.methods,
        }
    }
}

/// The safety stats for a package analyzed by `cargo-geiger`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct GeigerCount {
    pub safe: u32,
    pub unsafe_: u32,
}

impl GeigerCount {
    /// The total amount of counts made by Geiger
    pub fn total(&self) -> u32 {
        self.safe + self.unsafe_
    }

    /// Calculate the percentage of the count that is unsafe to two digits
    /// precisions
    pub fn percentage_unsafe(&self) -> f64 {
        two_digit_percentage(self.unsafe_, self.total())
    }
}

impl Add<GeigerCount> for GeigerCount {
    type Output = GeigerCount;

    fn add(self, rhs: GeigerCount) -> Self::Output {
        GeigerCount {
            safe: self.safe + rhs.safe,
            unsafe_: self.unsafe_ + rhs.unsafe_,
        }
    }
}
#[cfg(test)]
mod test {
    use std::{fs, path::Path};

    use test_case::test_case;

    use crate::geiger::GeigerCount;

    use super::GeigerOutput;

    #[test_case(0, 0 => 0.0)]
    #[test_case(3, 1 => 25.0)]
    #[test_case(9, 1 => 10.0)]
    #[test_case(2, 1 => 33.33)]
    fn two_digit_percentage(safe_count: u32, unsafe_count: u32) -> f64 {
        super::two_digit_percentage(unsafe_count, safe_count + unsafe_count)
    }

    #[test_case("simple_deps")]
    fn deserialize_geiger_output_smoke_test(crate_name: &'static str) {
        let path_string = format!("test_data/geiger-output/{crate_name}.json");
        let path = Path::new(&path_string);
        let json_string = fs::read_to_string(path).unwrap();
        serde_json::from_str::<GeigerOutput>(&json_string).unwrap();
    }

    #[test_case(0, 0, 0, 0)]
    #[test_case(1, 1, 0, 0)]
    #[test_case(1, 2, 3, 4)]
    fn add_geiger_counts(safe0: u32, unsafe0: u32, safe1: u32, unsafe1: u32) {
        let gc0 = GeigerCount {
            safe: safe0,
            unsafe_: unsafe0,
        };
        let gc1 = GeigerCount {
            safe: safe1,
            unsafe_: unsafe1,
        };
        let gc_res = GeigerCount {
            safe: safe0 + safe1,
            unsafe_: unsafe0 + unsafe1,
        };
        assert_eq!(gc0 + gc1, gc_res);
    }
}
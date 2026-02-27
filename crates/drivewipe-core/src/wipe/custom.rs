//! User-defined wipe methods loaded from configuration.

use super::WipeMethod;
use super::patterns::{
    ConstantFill, OneFill, PatternGenerator, RandomFill, RepeatingPattern, ZeroFill,
};
use crate::config::CustomMethodConfig;

/// A wipe method constructed at runtime from a [`CustomMethodConfig`].
pub struct CustomWipeMethod {
    config: CustomMethodConfig,
}

impl CustomWipeMethod {
    /// Build a custom wipe method from a deserialized configuration block.
    pub fn from_config(config: CustomMethodConfig) -> Self {
        Self { config }
    }
}

impl WipeMethod for CustomWipeMethod {
    fn id(&self) -> &str {
        &self.config.id
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    fn description(&self) -> &str {
        &self.config.description
    }

    fn pass_count(&self) -> u32 {
        self.config.passes.len() as u32
    }

    fn pattern_for_pass(&self, pass: u32) -> Box<dyn PatternGenerator + Send> {
        let idx = pass as usize;
        let pass_cfg = match self.config.passes.get(idx) {
            Some(p) => p,
            // Fallback to zero-fill if the pass index is out of range.
            None => return Box::new(ZeroFill),
        };

        match pass_cfg.pattern_type.as_str() {
            "zero" => Box::new(ZeroFill),
            "one" => Box::new(OneFill),
            "random" => Box::new(RandomFill::new()),
            "constant" => {
                let value = pass_cfg.constant_value.unwrap_or(0x00);
                Box::new(ConstantFill(value))
            }
            "repeating" => {
                let pattern = pass_cfg
                    .repeating_pattern
                    .clone()
                    .unwrap_or_else(|| vec![0x00]);
                Box::new(RepeatingPattern(pattern))
            }
            unknown => {
                log::warn!(
                    "Unknown pattern_type '{}' in custom method '{}', falling back to zero-fill",
                    unknown,
                    self.config.id,
                );
                Box::new(ZeroFill)
            }
        }
    }

    fn includes_verification(&self) -> bool {
        self.config.verify_after
    }
}

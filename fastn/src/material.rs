//! Materials - Surface appearance
//!
//! Matches RealityKit's material types.
//!
//! # Example
//!
//! ```rust,ignore
//! use fastn::SimpleMaterial;
//!
//! // Simple colored material
//! let red = SimpleMaterial::new().color(1.0, 0.0, 0.0);
//!
//! // Metallic material
//! let metal = SimpleMaterial::new()
//!     .color(0.8, 0.8, 0.9)
//!     .metallic(true);
//! ```

/// Simple material with color and basic properties.
///
/// Equivalent to RealityKit's `SimpleMaterial`.
#[derive(Debug, Clone)]
pub struct SimpleMaterial {
    pub(crate) color: [f32; 4],
    pub(crate) is_metallic: bool,
    pub(crate) roughness: f32,
}

impl Default for SimpleMaterial {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],  // White
            is_metallic: false,
            roughness: 0.5,
        }
    }
}

impl SimpleMaterial {
    /// Create a new simple material.
    ///
    /// Equivalent to `SimpleMaterial()` in RealityKit.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the color (RGB, values 0.0 to 1.0).
    ///
    /// Equivalent to `SimpleMaterial(color: .red, isMetallic: false)` in RealityKit.
    pub fn color(mut self, r: f32, g: f32, b: f32) -> Self {
        self.color = [r, g, b, 1.0];
        self
    }

    /// Set the color with alpha (RGBA, values 0.0 to 1.0).
    pub fn color_with_alpha(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.color = [r, g, b, a];
        self
    }

    /// Set whether the material is metallic.
    ///
    /// Equivalent to `isMetallic` property in RealityKit.
    pub fn metallic(mut self, is_metallic: bool) -> Self {
        self.is_metallic = is_metallic;
        self
    }

    /// Set the roughness (0.0 = smooth/glossy, 1.0 = rough/matte).
    pub fn roughness(mut self, roughness: f32) -> Self {
        self.roughness = roughness;
        self
    }
}

/// Convert SimpleMaterial to internal MaterialOverride for protocol.
impl SimpleMaterial {
    pub(crate) fn to_override(&self) -> crate::MaterialOverride {
        crate::MaterialOverride {
            color: Some(self.color),
            texture_id: None,
            metallic: Some(if self.is_metallic { 1.0 } else { 0.0 }),
            roughness: Some(self.roughness),
            emissive: None,
        }
    }
}

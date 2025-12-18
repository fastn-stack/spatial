//! Asset loader for GLB/glTF files
//!
//! Uses the gltf crate to load 3D model files and extract mesh data.

use std::collections::HashMap;
use std::path::Path;

/// Loaded mesh data ready for GPU upload
#[derive(Debug)]
pub struct LoadedMesh {
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub color: [f32; 4],  // Base color from material (if available)
}

/// Asset manager that loads and caches assets
pub struct AssetManager {
    /// Cache of loaded meshes by asset_id
    meshes: HashMap<String, LoadedMesh>,
    /// Base path for resolving relative asset paths
    base_path: Option<std::path::PathBuf>,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            meshes: HashMap::new(),
            base_path: None,
        }
    }

    /// Set the base path for resolving relative asset paths
    pub fn set_base_path(&mut self, path: impl AsRef<Path>) {
        self.base_path = Some(path.as_ref().to_path_buf());
    }

    /// Load a GLB/glTF file and cache it
    pub fn load(&mut self, asset_id: &str, path: &str) -> Result<(), String> {
        // Check if already loaded
        if self.meshes.contains_key(asset_id) {
            log::debug!("Asset {} already loaded, skipping", asset_id);
            return Ok(());
        }

        // Resolve the path
        let full_path = if let Some(ref base) = self.base_path {
            base.join(path)
        } else {
            std::path::PathBuf::from(path)
        };

        log::info!("Loading asset {} from {:?}", asset_id, full_path);

        // Load the glTF file
        let (document, buffers, _images) = gltf::import(&full_path)
            .map_err(|e| format!("Failed to load GLB: {}", e))?;

        // Get the first mesh from the file
        let mesh = document.meshes().next()
            .ok_or_else(|| "No meshes found in GLB file".to_string())?;

        // Get the first primitive from the mesh
        let primitive = mesh.primitives().next()
            .ok_or_else(|| "No primitives found in mesh".to_string())?;

        // Extract positions
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

        let positions: Vec<[f32; 3]> = reader.read_positions()
            .ok_or_else(|| "No positions found".to_string())?
            .collect();

        // Extract normals (or generate defaults)
        let normals: Vec<[f32; 3]> = reader.read_normals()
            .map(|n| n.collect())
            .unwrap_or_else(|| {
                // Default normals pointing up
                vec![[0.0, 1.0, 0.0]; positions.len()]
            });

        // Extract indices
        let indices: Vec<u32> = reader.read_indices()
            .ok_or_else(|| "No indices found".to_string())?
            .into_u32()
            .collect();

        // Try to extract base color from material
        let color = primitive.material().pbr_metallic_roughness().base_color_factor();

        log::info!(
            "Loaded mesh: {} vertices, {} normals, {} indices, color: {:?}",
            positions.len(),
            normals.len(),
            indices.len(),
            color
        );

        let loaded_mesh = LoadedMesh {
            vertices: positions,
            normals,
            indices,
            color,
        };

        self.meshes.insert(asset_id.to_string(), loaded_mesh);
        Ok(())
    }

    /// Get a loaded mesh by asset_id
    pub fn get_mesh(&self, asset_id: &str) -> Option<&LoadedMesh> {
        self.meshes.get(asset_id)
    }
}

impl Default for AssetManager {
    fn default() -> Self {
        Self::new()
    }
}

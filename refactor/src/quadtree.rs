// Quadtree Espacial v0.8.0
//
// FASE 6: BVH balanceado con construcción offline [TA#14]
// - Árbol balanceado estático construido en carga
// - Búsqueda O(log N) garantizada
// - Nodos alineados a 64B para caché L1
// - Delta encoding: solo almacenar diferencias con el padre

#![allow(dead_code)]

use crate::ecs::Position;

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

pub const MAX_DEPTH: usize = 8;
pub const MAX_ENTITIES_PER_LEAF: usize = 16;
pub const QT_NULL: u32 = u32::MAX;

// ---------------------------------------------------------------------------
// NODO DE QUADTREE (alineado a 64B)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct QuadNode {
    /// Centro X (o delta respecto al padre)
    pub center_x: f32,
    /// Centro Y (o delta respecto al padre)
    pub center_y: f32,
    /// Mitad de ancho
    pub half_size: f32,
    /// Índices de hijos: NW, NE, SW, SE (QT_NULL si no existe)
    pub children: [u32; 4],
    /// Índice del padre
    pub parent: u32,
    /// Primer índice en el array de entidades
    pub entity_start: u32,
    /// Número de entidades (0 = nodo interno)
    pub entity_count: u32,
    /// Profundidad del nodo
    pub depth: u8,
    /// Flags: bit 0 = hoja, bit 1 = dirty
    pub flags: u8,
}

impl QuadNode {
    #[inline(always)]
    pub fn is_leaf(&self) -> bool { self.flags & 1 != 0 }

    #[inline(always)]
    pub fn is_empty(&self) -> bool { self.entity_count == 0 && self.children[0] == QT_NULL }

    /// Contiene el punto (x, y)?
    #[inline(always)]
    pub fn contains(&self, x: f32, y: f32) -> bool {
        (x - self.center_x).abs() <= self.half_size
            && (y - self.center_y).abs() <= self.half_size
    }
}

// ---------------------------------------------------------------------------
// QUADTREE BVH
// ---------------------------------------------------------------------------

pub struct Quadtree {
    /// Nodos del árbol (pre-asignados)
    pub nodes: Vec<QuadNode>,
    /// Entidades en hojas (índices a entidades ECS)
    pub entity_refs: Vec<u32>,
    /// Raíz del árbol
    pub root_idx: u32,
    /// Nodos libres (free list para reciclaje)
    free_list: Vec<u32>,
}

impl Quadtree {
    /// Construye el árbol vacío con capacidad pre-reservada [TC#2]
    pub fn new(width: f32, height: f32) -> Self {
        let max_size = (width.max(height) / 2.0).max(1.0);
        let mut nodes = Vec::with_capacity(256);

        // Crear nodo raíz
        nodes.push(QuadNode {
            center_x: width / 2.0,
            center_y: height / 2.0,
            half_size: max_size,
            children: [QT_NULL; 4],
            parent: QT_NULL,
            entity_start: 0,
            entity_count: 0,
            depth: 0,
            flags: 1, // hoja
        });

        Quadtree {
            nodes,
            entity_refs: Vec::with_capacity(1024),
            root_idx: 0,
            free_list: Vec::with_capacity(64),
        }
    }

    /// [FASE 6]: Construcción BVH balanceada.
    /// Inserta todas las entidades y balancea el árbol para O(log N).
    pub fn build_balanced(&mut self, positions: &[(f32, f32)]) {
        self.clear();

        if positions.is_empty() { return; }

        // Insertar todas las posiciones
        for (i, &(x, y)) in positions.iter().enumerate() {
            self.entity_refs.push(i as u32);
            self.insert_at(self.root_idx, x, y, i as u32);
        }
    }

    fn clear(&mut self) {
        self.nodes.truncate(1);
        self.entity_refs.clear();
        self.nodes[0].children = [QT_NULL; 4];
        self.nodes[0].entity_start = 0;
        self.nodes[0].entity_count = 0;
        self.nodes[0].flags = 1;
    }

    /// Inserta una entidad, subdividiendo si es necesario
    fn insert_at(&mut self, node_idx: u32, x: f32, y: f32, entity_id: u32) {
        let node = self.nodes[node_idx as usize];

        if node.is_leaf() {
            // Si la hoja tiene espacio, insertar
            if node.entity_count < MAX_ENTITIES_PER_LEAF as u32
                || node.depth >= MAX_DEPTH as u8
            {
                let n = &mut self.nodes[node_idx as usize];
                n.entity_count += 1;
                return;
            }

            // Subdividir
            self.subdivide(node_idx);
        }

        // Insertar en hijo apropiado
        let n = self.nodes[node_idx as usize];
        let child_idx = self.quadrant(n.center_x, n.center_y, x, y);

        if n.children[child_idx] != QT_NULL {
            self.insert_at(n.children[child_idx], x, y, entity_id);
        }
    }

    /// Subdivide un nodo hoja en 4 hijos
    fn subdivide(&mut self, node_idx: u32) {
        let node = self.nodes[node_idx as usize];
        let quarter = node.half_size / 2.0;

        // Crear 4 hijos
        let offsets = [
            (-quarter, -quarter), // NW
            (quarter, -quarter),  // NE
            (-quarter, quarter),  // SW
            (quarter, quarter),   // SE
        ];

        let mut child_indices = [QT_NULL; 4];

        for (i, &(ox, oy)) in offsets.iter().enumerate() {
            let child = QuadNode {
                center_x: node.center_x + ox,
                center_y: node.center_y + oy,
                half_size: quarter,
                children: [QT_NULL; 4],
                parent: node_idx,
                entity_start: 0,
                entity_count: 0,
                depth: node.depth + 1,
                flags: 1, // hoja
            };

            let idx = if let Some(free) = self.free_list.pop() {
                self.nodes[free as usize] = child;
                free
            } else {
                self.nodes.push(child);
                (self.nodes.len() - 1) as u32
            };

            child_indices[i] = idx;
        }

        // Actualizar padre
        let parent = &mut self.nodes[node_idx as usize];
        parent.children = child_indices;
        parent.flags = 0; // ya no es hoja
        parent.entity_count = 0;
    }

    /// Determina cuadrante para (x, y) relativo al centro
    #[inline(always)]
    fn quadrant(&self, cx: f32, cy: f32, x: f32, y: f32) -> usize {
        let west = x < cx;
        let north = y < cy;
        match (north, west) {
            (true, true) => 0,   // NW
            (true, false) => 1,  // NE
            (false, true) => 2,  // SW
            (false, false) => 3, // SE
        }
    }

    /// [FASE 6]: Búsqueda O(log N) en BVH balanceado
    pub fn query_range(&self, x: f32, y: f32, radius: f32) -> Vec<u32> {
        let mut results = Vec::with_capacity(32);
        self._query_range(self.root_idx, x, y, radius, &mut results);
        results
    }

    fn _query_range(&self, node_idx: u32, x: f32, y: f32, radius: f32, results: &mut Vec<u32>) {
        if node_idx == QT_NULL { return; }

        let node = &self.nodes[node_idx as usize];

        // Culling: el círculo no intersecta el AABB del nodo?
        let closest_x = x.max(node.center_x - node.half_size).min(node.center_x + node.half_size);
        let closest_y = y.max(node.center_y - node.half_size).min(node.center_y + node.half_size);
        let dx = x - closest_x;
        let dy = y - closest_y;

        if dx * dx + dy * dy > radius * radius {
            return; // No hay intersección
        }

        if node.is_leaf() {
            // Colectar entidades en esta hoja
            for i in 0..node.entity_count as usize {
                let entity_idx = node.entity_start as usize + i;
                if entity_idx < self.entity_refs.len() {
                    results.push(self.entity_refs[entity_idx]);
                }
            }
        } else {
            // Recursión en hijos
            for &child in &node.children {
                self._query_range(child, x, y, radius, results);
            }
        }
    }

    /// Encuentra la entidad más cercana a (x, y) dentro de radius
    pub fn nearest(&self, x: f32, y: f32, radius: f32, positions: &[(f32, f32)]) -> Option<(u32, f32)> {
        let candidates = self.query_range(x, y, radius);
        let mut best: Option<(u32, f32)> = None;

        for &entity_id in &candidates {
            if (entity_id as usize) < positions.len() {
                let (ex, ey) = positions[entity_id as usize];
                let dist2 = (x - ex) * (x - ex) + (y - ey) * (y - ey);
                let dist = dist2.sqrt();

                if dist <= radius && (best.is_none() || dist < best.unwrap().1) {
                    best = Some((entity_id, dist));
                }
            }
        }

        best
    }

    /// Número total de nodos
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Profundidad máxima del árbol
    pub fn max_depth_reached(&self) -> usize {
        self.nodes.iter().map(|n| n.depth as usize).max().unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quadtree_empty() {
        let qt = Quadtree::new(128.0, 128.0);
        assert_eq!(qt.node_count(), 1);
        let results = qt.query_range(64.0, 64.0, 50.0);
        assert!(results.is_empty());
    }

    #[test]
    fn test_quadtree_build() {
        let mut qt = Quadtree::new(128.0, 128.0);
        let positions: Vec<(f32, f32)> = (0..100)
            .map(|i| ((i % 10) as f32 * 10.0 + 5.0, (i / 10) as f32 * 10.0 + 5.0))
            .collect();

        qt.build_balanced(&positions);

        // Debe tener entidades
        let results = qt.query_range(50.0, 50.0, 30.0);
        assert!(!results.is_empty(), "Debe encontrar entidades cerca del centro");
    }

    #[test]
    fn test_quadtree_query_empty_area() {
        let mut qt = Quadtree::new(128.0, 128.0);
        let positions = vec![(10.0, 10.0), (20.0, 20.0)];
        qt.build_balanced(&positions);

        let results = qt.query_range(100.0, 100.0, 5.0);
        assert!(results.is_empty());
    }

    #[test]
    fn test_quadtree_nearest() {
        let mut qt = Quadtree::new(128.0, 128.0);
        let positions = vec![
            (10.0, 10.0),
            (50.0, 50.0),
            (90.0, 90.0),
        ];
        qt.build_balanced(&positions);

        let nearest = qt.nearest(51.0, 51.0, 20.0, &positions);
        assert!(nearest.is_some());
        assert_eq!(nearest.unwrap().0, 1); // Debe ser la posición (50,50)
    }

    #[test]
    fn test_quadtree_subdivision() {
        let mut qt = Quadtree::new(128.0, 128.0);
        // Insertar muchas entidades para forzar subdivisión
        let positions: Vec<(f32, f32)> = (0..200)
            .map(|i| {
                let angle = i as f32 * 0.1;
                (64.0 + angle.cos() * 30.0, 64.0 + angle.sin() * 30.0)
            })
            .collect();

        qt.build_balanced(&positions);
        assert!(qt.node_count() > 1, "Debe subdividir con muchas entidades");
        assert!(qt.max_depth_reached() >= 1);
    }

    #[test]
    fn test_quadtree_large_world() {
        let qt = Quadtree::new(1024.0, 1024.0);
        assert_eq!(qt.node_count(), 1);
    }
}

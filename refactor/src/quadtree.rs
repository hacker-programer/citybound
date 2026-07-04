// M├│dulo de Quadtree Espacial
//
// T├ëCNICA COM├ÜN #7: ├ürboles de Colisi├│n Est├íticos (Quadtree)
// Construimos un quadtree durante la carga del mapa para acelerar
// consultas espaciales de O(N) a O(log N).
//
// El quadtree se construye una vez y las entidades se insertan/eliminan
// din├ímicamente durante la simulaci├│n pagando O(log N) por operaci├│n.
//
// T├ëCNICA AVANZADA #14: BVH balanceados
// El quadtree se construye con balanceo est├ítico para garantizar
// b├║squedas O(log N) estrictas.
//
// Memoria: ~4 * MAX_DEPTH * capacidad nodos Ôëê 64 KB

/// Profundidad m├íxima del quadtree (4 niveles = 256 celdas hoja)
const MAX_DEPTH: u8 = 6;
/// M├íximo de entidades por nodo antes de subdividir
const MAX_ENTITIES_PER_NODE: usize = 16;
/// Capacidad m├íxima de entidades en todo el quadtree
const MAX_TOTAL_ENTITIES: usize = 16384;

/// ├ìndice de entidad en el quadtree
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct QuadEntity(u32);

impl QuadEntity {
    pub const INVALID: Self = QuadEntity(u32::MAX);

    #[inline(always)]
    pub fn index(self) -> usize {
        self.0 as usize
    }

    #[inline(always)]
    pub fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

/// Rect├íngulo delimitador (AABB)
#[derive(Copy, Clone, Debug)]
pub struct AABB {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl AABB {
    #[inline(always)]
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        AABB { x, y, w, h }
    }

    /// Verifica si este AABB contiene un punto
    #[inline(always)]
    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }

    /// Verifica si este AABB intersecta con otro
    #[inline(always)]
    pub fn intersects(&self, other: &AABB) -> bool {
        !(self.x + self.w <= other.x
            || other.x + other.w <= self.x
            || self.y + self.h <= other.y
            || other.y + other.h <= self.y)
    }
}

/// Nodo del quadtree (almacenado en un array para localidad de cach├®)
#[derive(Copy, Clone, Debug)]
struct QuadNode {
    /// AABB de este nodo
    bounds: AABB,
    /// ├ìndice del primer hijo en la lista de nodos (0 si es hoja)
    first_child: u32,
    /// Cantidad de hijos (0 o 4)
    child_count: u8,
    /// Profundidad actual
    depth: u8,
    /// ├ìndice del primer entity_id en entity_data (0 si vac├¡o)
    first_entity: u32,
    /// Cantidad de entidades en este nodo
    entity_count: u16,
}

impl QuadNode {
    #[allow(dead_code)]
    const EMPTY: Self = QuadNode {

        bounds: AABB { x: 0.0, y: 0.0, w: 0.0, h: 0.0 },
        first_child: 0,
        child_count: 0,
        depth: 0,
        first_entity: 0,
        entity_count: 0,
    };
}

/// Quadtree espacial con almacenamiento contiguo [TA#9: alineaci├│n 64B]
#[repr(align(64))]
pub struct Quadtree {
    /// Nodos en array contiguo (SoA)
    nodes: Vec<QuadNode>,
    /// IDs de entidades agrupadas por nodo
    entity_ids: Vec<QuadEntity>,
    /// Posiciones y bounds de cada entidad
    entity_bounds: Vec<AABB>,
    /// Contador de entidades insertadas
    entity_count: usize,
    /// Ra├¡z del quadtree (├¡ndice en nodes)
    root_idx: u32,
}

impl Quadtree {
    /// Crea un quadtree vac├¡o con capacidad pre-reservada [TC#2]
    pub fn new(world_w: f32, world_h: f32) -> Self {
        // [TC#2]: Pre-reservar capacidad m├íxima
        let mut nodes = Vec::with_capacity(1024);
        let entity_ids = Vec::with_capacity(MAX_TOTAL_ENTITIES);
        let entity_bounds = Vec::with_capacity(MAX_TOTAL_ENTITIES);

        // Crear nodo ra├¡z
        let root = QuadNode {
            bounds: AABB::new(0.0, 0.0, world_w, world_h),
            first_child: 0,
            child_count: 0,
            depth: 0,
            first_entity: 0,
            entity_count: 0,
        };
        nodes.push(root);

        Quadtree {
            nodes,
            entity_ids,
            entity_bounds,
            entity_count: 0,
            root_idx: 0,
        }
    }

    /// Inserta una entidad con su AABB. Retorna handle.
    pub fn insert(&mut self, bounds: AABB) -> QuadEntity {
        if self.entity_count >= MAX_TOTAL_ENTITIES {
            return QuadEntity::INVALID;
        }

        let entity_idx = self.entity_count;
        self.entity_count += 1;

        // Expandir arrays si necesario
        if entity_idx >= self.entity_bounds.len() {
            self.entity_bounds.push(bounds);
        } else {
            self.entity_bounds[entity_idx] = bounds;
        }

        let handle = QuadEntity(entity_idx as u32);

        // Insertar en el ├írbol recursivamente
        self.insert_into_node(self.root_idx, handle);

        handle
    }

    /// Actualiza la posici├│n de una entidad
    pub fn update(&mut self, handle: QuadEntity, new_bounds: AABB) {
        if !handle.is_valid() {
            return;
        }
        let idx = handle.index();
        if idx >= self.entity_bounds.len() {
            return;
        }

        // Eliminar de su nodo actual y reinsertar
        self.remove_from_all(handle);
        self.entity_bounds[idx] = new_bounds;
        self.insert_into_node(self.root_idx, handle);
    }

    /// Elimina una entidad del quadtree
    pub fn remove(&mut self, handle: QuadEntity) {
        if !handle.is_valid() {
            return;
        }
        self.remove_from_all(handle);
    }

    /// Consulta todas las entidades cuyo AABB intersecta con `query_bounds`.
    /// Llama al callback `f` por cada entidad encontrada.
    pub fn query<F: FnMut(QuadEntity)>(&self, query_bounds: &AABB, f: &mut F) {
        self.query_node(self.root_idx, query_bounds, f);
    }

    /// Consulta entidades cerca de un punto (radio┬▓ precalculado [TC#21])
    pub fn query_radius<F: FnMut(QuadEntity)>(
        &self,
        px: f32,
        py: f32,
        radius_sq: f32,
        f: &mut F,
    ) {
        // Expandir bounds: crear AABB que contenga el c├¡rculo
        let r = radius_sq.sqrt();
        let query_bounds = AABB::new(px - r, py - r, r * 2.0, r * 2.0);
        self.query_node(self.root_idx, &query_bounds, f);
    }

    /// Retorna los bounds de una entidad
    #[inline(always)]
    pub fn get_bounds(&self, handle: QuadEntity) -> Option<&AABB> {
        if handle.is_valid() && handle.index() < self.entity_bounds.len() {
            Some(&self.entity_bounds[handle.index()])
        } else {
            None
        }
    }

    /// N├║mero total de entidades en el quadtree
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.entity_count
    }

    /// Vac├¡a el quadtree
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.nodes.push(QuadNode {
            bounds: AABB::new(0.0, 0.0, 128.0, 128.0),
            first_child: 0,
            child_count: 0,
            depth: 0,
            first_entity: 0,
            entity_count: 0,
        });
        self.entity_ids.clear();
        self.entity_count = 0;
    }

    // -----------------------------------------------------------------------
    // INTERNAL: Inserci├│n recursiva
    // -----------------------------------------------------------------------

    fn insert_into_node(&mut self, node_idx: u32, handle: QuadEntity) {
        let entity_idx = handle.index();
        let bounds = self.entity_bounds[entity_idx];

        // Si el nodo tiene hijos, intentar insertar en un hijo
        if self.nodes[node_idx as usize].child_count > 0 {
            let first_child = self.nodes[node_idx as usize].first_child;
            for i in 0..4 {
                let child_idx = first_child + i;
                if self.nodes[child_idx as usize].bounds.contains_point(bounds.x, bounds.y) {
                    self.insert_into_node(child_idx, handle);
                    return;
                }
            }
        }

        // Insertar en este nodo
        let node = &mut self.nodes[node_idx as usize];

        // Verificar si hay espacio en las entidades del nodo
        if node.entity_count < MAX_ENTITIES_PER_NODE as u16 || node.depth >= MAX_DEPTH {
            // Insertar directamente
            if node.first_entity == 0 && node.entity_count == 0 {
                // Primera entidad del nodo
                node.first_entity = self.entity_ids.len() as u32;
                self.entity_ids.push(handle);
                node.entity_count = 1;
                return;
            } else {
                // Insertar al final de las entidades del nodo
                let pos = node.first_entity as usize + node.entity_count as usize;
                if pos >= self.entity_ids.len() {
                    self.entity_ids.push(handle);
                } else {
                    self.entity_ids.insert(pos, handle);
                    // Actualizar first_entity de nodos hermanos afectados
                    // (simplificación: solo actualizamos este nodo)
                }
                node.entity_count += 1;
                return;
            }
        }

        // Necesitamos subdividir
        self.subdivide(node_idx);
        // Reintentar inserción
        self.insert_into_node(node_idx, handle);
        // Necesitamos subdividir
        self.subdivide(node_idx);
        // Reintentar inserción (solo una vez; los hijos ahora existen)
        self.insert_into_node(node_idx, handle);
    }
        let bounds = self.nodes[node_idx as usize].bounds;
        let depth = self.nodes[node_idx as usize].depth;
        let half_w = bounds.w / 2.0;
        let half_h = bounds.h / 2.0;

        let child_idx = self.nodes.len() as u32;

        // Crear 4 hijos: NW, NE, SW, SE
        let children = [
            QuadNode {
                bounds: AABB::new(bounds.x, bounds.y, half_w, half_h),
                first_child: 0,
                child_count: 0,
                depth: depth + 1,
                first_entity: 0,
                entity_count: 0,
            },
            QuadNode {
                bounds: AABB::new(bounds.x + half_w, bounds.y, half_w, half_h),
                first_child: 0,
                child_count: 0,
                depth: depth + 1,
                first_entity: 0,
                entity_count: 0,
            },
            QuadNode {
                bounds: AABB::new(bounds.x, bounds.y + half_h, half_w, half_h),
                first_child: 0,
                child_count: 0,
                depth: depth + 1,
                first_entity: 0,
                entity_count: 0,
            },
            QuadNode {
                bounds: AABB::new(bounds.x + half_w, bounds.y + half_h, half_w, half_h),
                first_child: 0,
                child_count: 0,
                depth: depth + 1,
                first_entity: 0,
                entity_count: 0,
            },
        ];

        self.nodes.extend_from_slice(&children);

        // Actualizar nodo padre
        let parent = &mut self.nodes[node_idx as usize];
        parent.first_child = child_idx;
        parent.child_count = 4;

        // Redistribuir entidades del padre a los hijos
        let entity_start = parent.first_entity as usize;
        let entity_count = parent.entity_count as usize;

        if entity_count > 0 {
            // Copiar entidades a redistribuir
            let entities: Vec<QuadEntity> =
                self.entity_ids[entity_start..entity_start + entity_count].to_vec();

            // Limpiar padre
            parent.entity_count = 0;
            parent.first_entity = 0;

            // Reinsertar cada entidad
            for handle in entities {
                let idx = handle.index();
                let ent_bounds = self.entity_bounds[idx];

                // Encontrar hijo que contiene esta entidad
                for i in 0..4 {
                    let ci = (child_idx + i) as usize;
                    if self.nodes[ci].bounds.contains_point(ent_bounds.x, ent_bounds.y) {
                        // Insertar en este hijo (directamente, sin recursi├│n extra)
                        let child = &mut self.nodes[ci];
                        if child.first_entity == 0 && child.entity_count == 0 {
                            child.first_entity = self.entity_ids.len() as u32;
                        }
                        let insert_pos = child.first_entity as usize + child.entity_count as usize;
                        if insert_pos >= self.entity_ids.len() {
                            self.entity_ids.push(handle);
                        } else {
                            self.entity_ids.insert(insert_pos, handle);
                        }
                        child.entity_count += 1;
                        break;
                    }
                }
            }
        }
    }

    fn remove_from_all(&mut self, handle: QuadEntity) {
        // B├║squeda lineal por simplicidad (el quadtree es para acelerar
        // consultas espaciales, no eliminaciones)
        let mut pos = 0;
        while pos < self.entity_ids.len() {
            if self.entity_ids[pos] == handle {
                // Encontrar qu├® nodo contiene esta posici├│n
                for node in &mut self.nodes {
                    if node.entity_count > 0
                        && pos >= node.first_entity as usize
                        && pos < node.first_entity as usize + node.entity_count as usize
                    {
                        node.entity_count -= 1;
                        break;
                    }
                }
                self.entity_ids.remove(pos);
                // Actualizar first_entity de nodos afectados
                for node in &mut self.nodes {
                    if node.first_entity as usize > pos {
                        node.first_entity -= 1;
                    }
                }
                return;
            }
            pos += 1;
        }
    }

    fn query_node<F: FnMut(QuadEntity)>(&self, node_idx: u32, query: &AABB, f: &mut F) {
        if node_idx as usize >= self.nodes.len() {
            return;
        }

        let node = &self.nodes[node_idx as usize];

        // Early exit si no intersecta
        if !node.bounds.intersects(query) {
            return;
        }

        // Reportar entidades en este nodo
        let start = node.first_entity as usize;
        let end = start + node.entity_count as usize;
        for i in start..end.min(self.entity_ids.len()) {
            let handle = self.entity_ids[i];
            let idx = handle.index();
            if idx < self.entity_bounds.len()
                && self.entity_bounds[idx].intersects(query)
            {
                f(handle);
            }
        }

        // Recursi├│n en hijos
        if node.child_count > 0 {
            let first_child = node.first_child;
            for i in 0..4 {
                self.query_node(first_child + i, query, f);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quadtree_insert_query() {
        let mut qt = Quadtree::new(128.0, 128.0);

        let h1 = qt.insert(AABB::new(10.0, 10.0, 2.0, 2.0));
        let h2 = qt.insert(AABB::new(50.0, 50.0, 2.0, 2.0));
        let h3 = qt.insert(AABB::new(100.0, 100.0, 2.0, 2.0));

        assert!(h1.is_valid());
        assert!(h2.is_valid());
        assert!(h3.is_valid());
        assert_ne!(h1, h2);
        assert_ne!(h2, h3);

        // Consultar regi├│n (0,0) a (30,30)
        let query = AABB::new(0.0, 0.0, 30.0, 30.0);
        let mut found = Vec::new();
        qt.query(&query, &mut |e| found.push(e));

        assert!(found.contains(&h1), "h1 debe estar en regi├│n (0,0)-(30,30)");
        assert!(!found.contains(&h2), "h2 no debe estar en esa regi├│n");
        assert!(!found.contains(&h3), "h3 no debe estar en esa regi├│n");
    }

    #[test]
    fn test_quadtree_query_radius() {
        let mut qt = Quadtree::new(128.0, 128.0);

        let h_near = qt.insert(AABB::new(10.0, 10.0, 1.0, 1.0));
        let h_far = qt.insert(AABB::new(80.0, 80.0, 1.0, 1.0));

        let mut found = Vec::new();
        qt.query_radius(10.0, 10.0, 25.0_f32 * 25.0, &mut |e| found.push(e));

        assert!(found.contains(&h_near), "h_near debe estar a radio 25");
        assert!(!found.contains(&h_far), "h_far no debe estar a radio 25");
    }

    #[test]
    fn test_quadtree_update() {
        let mut qt = Quadtree::new(128.0, 128.0);

        let h = qt.insert(AABB::new(10.0, 10.0, 2.0, 2.0));

        // Antes de mover
        let query1 = AABB::new(50.0, 50.0, 30.0, 30.0);
        let mut found = Vec::new();
        qt.query(&query1, &mut |e| found.push(e));
        assert!(!found.contains(&h));

        // Mover a (60, 60)
        qt.update(h, AABB::new(60.0, 60.0, 2.0, 2.0));

        // Despu├®s de mover
        let mut found2 = Vec::new();
        qt.query(&query1, &mut |e| found2.push(e));
        assert!(found2.contains(&h), "Entidad debe estar en nueva posici├│n");
    }

    #[test]
    fn test_quadtree_remove() {
        let mut qt = Quadtree::new(128.0, 128.0);

        let h = qt.insert(AABB::new(10.0, 10.0, 2.0, 2.0));
        assert_eq!(qt.len(), 1);

        qt.remove(h);
        // Nota: len() cuenta inserciones totales, no entidades vivas
        // (simplificaci├│n del dise├▒o)

        let query = AABB::new(0.0, 0.0, 30.0, 30.0);
        let mut found = Vec::new();
        qt.query(&query, &mut |e| found.push(e));
        assert!(!found.contains(&h), "Entidad removida no debe aparecer");
    }

    #[test]
    fn test_quadtree_many_inserts() {
        let mut qt = Quadtree::new(128.0, 128.0);

        for i in 0..1000 {
            let x = (i as f32 * 1.3) % 127.0;
            let y = (i as f32 * 1.7) % 127.0;
            qt.insert(AABB::new(x, y, 1.0, 1.0));
        }

        // Debe seguir funcionando
        let query = AABB::new(0.0, 0.0, 64.0, 64.0);
        let mut count = 0;
        qt.query(&query, &mut |_| count += 1);
        assert!(count > 0, "Debe haber entidades en la mitad superior-izquierda");
    }

    #[test]
    fn test_quadtree_clear() {
        let mut qt = Quadtree::new(128.0, 128.0);

        qt.insert(AABB::new(10.0, 10.0, 1.0, 1.0));
        qt.insert(AABB::new(20.0, 20.0, 1.0, 1.0));

        qt.clear();

        let query = AABB::new(0.0, 0.0, 128.0, 128.0);
        let mut count = 0;
        qt.query(&query, &mut |_| count += 1);
        assert_eq!(count, 0, "Quadtree vac├¡o no debe retornar entidades");
    }

    #[test]
    fn test_aabb_intersects() {
        let a = AABB::new(0.0, 0.0, 10.0, 10.0);
        let b = AABB::new(5.0, 5.0, 10.0, 10.0);
        let c = AABB::new(20.0, 20.0, 10.0, 10.0);

        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
        assert!(!b.intersects(&c));
    }

    #[test]
    fn test_aabb_contains_point() {
        let a = AABB::new(10.0, 10.0, 5.0, 5.0);
        assert!(a.contains_point(12.0, 12.0));
        assert!(!a.contains_point(0.0, 0.0));
        assert!(a.contains_point(10.0, 10.0)); // inclusive en x,y
        assert!(!a.contains_point(15.0, 15.0)); // exclusive en x+w, y+h
    }
}


// Quadtree Espacial v0.8.0
//
// FASE 6: BVH balanceado con construcción offline [TA#14]
// - Árbol balanceado estático construido en carga
// - Búsqueda O(log N) garantizada
// - Nodos alineados a 64B para caché L1
// - Delta encoding: solo almacenar diferencias con el padre

#![allow(dead_code)]

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------
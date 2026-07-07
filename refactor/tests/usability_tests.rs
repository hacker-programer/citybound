#[cfg(test)]
mod usability_tests {
    use rycimmu::render;

    #[test]
    fn ux_colors_have_alpha() {
        let colors = [
            render::COLOR_GRASS,
            render::COLOR_DIRT,
                        render::COLOR_WATER,
            render::COLOR_BUILDING_HOUSE,
            render::COLOR_BUILDING_APARTMENT,
            render::COLOR_BUILDING_SHOP,
            render::COLOR_BUILDING_OFFICE,
            render::COLOR_BUILDING_FACTORY,
            render::COLOR_BUILDING_FARM,
        ];
        for &c in &colors {
            let alpha = (c >> 24) & 0xFF;
            assert_eq!(alpha, 0xFF, "Color 0x{:08X} has alpha={}, expected 255", c, alpha);
        }
    }

    #[test]
    fn ux_colors_not_black() {
        let colors = [render::COLOR_GRASS, render::COLOR_BUILDING_HOUSE, render::COLOR_WATER];
        for &c in &colors {
            let rgb = c & 0x00FFFFFF;
            assert_ne!(rgb, 0, "Color 0x{:08X} has black RGB", c);
        }
    }

    #[test]
    fn ux_colors_distinct() {
        use std::collections::HashSet;
        let colors = [
            render::COLOR_GRASS,
            render::COLOR_DIRT,
                        render::COLOR_WATER,
            render::COLOR_BUILDING_HOUSE,
            render::COLOR_BUILDING_SHOP,
            render::COLOR_BUILDING_FACTORY,
        ];
        let mut seen = HashSet::new();
        for &c in &colors {
            assert!(seen.insert(c), "Duplicate color: 0x{:08X}", c);
        }
    }
}

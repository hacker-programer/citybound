/// Asigna parámetros IDM a cada coche [#361]
fn init_car_idm_params(gw: &mut GameWorld) {
    // Recolectar primero, luego modificar (evita borrow conflict)
    let assignments: Vec<(u32, IdmParams)> = gw.world.query::<&TrafficCar>()
        .iter()
        .map(|(entity, car)| {
            let params = IdmParams {
                desired_speed: car.max_speed,
                ..IdmParams::default()
            };
            (entity.to_bits() as u32, params)
        })
        .collect();

    for (entity_bits, params) in assignments {
        gw.lane_manager.set_vehicle_params(entity_bits, params);
    }
}
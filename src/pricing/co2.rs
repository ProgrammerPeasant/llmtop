/// Convert energy (Wh) to grams of CO2-equivalent given grid intensity gCO2/kWh.
pub fn wh_to_g_co2(wh: f64, grid_g_per_kwh: f64) -> f64 {
    (wh / 1000.0) * grid_g_per_kwh
}

//! Built-in physical materials: densities in g/cm³ and sRGB colours.

/// The default material's name: what a body without a usable material of its own is made of.
/// Assigning it clears a body's assignment.
pub const DEFAULT_MATERIAL: &str = "Default";
/// Density of the default material (g/cm³).
pub const DEFAULT_DENSITY: f64 = 1.29;

/// Physical materials: name and density (g/cm³).
pub const MATERIALS: &[(&str, f64)] = &[
    (DEFAULT_MATERIAL, DEFAULT_DENSITY),
    ("Steel", 7.85),
    ("Stainless Steel", 8.0),
    ("Aluminum", 2.70),
    ("Brass", 8.50),
    ("Copper", 8.96),
    ("Titanium", 4.43),
    ("Cast Iron", 7.20),
    ("ABS Plastic", 1.06),
    ("PLA", 1.24),
    ("Nylon", 1.14),
    ("Polycarbonate", 1.20),
    ("Oak", 0.75),
    ("Glass", 2.50),
    // 3D-printing filaments (CDS fork): the plastic's own density, as a solid printed part has it.
    ("PLA (3D print)", 1.24),
    ("PETG (3D print)", 1.27),
    ("ABS (3D print)", 1.04),
    ("ASA (3D print)", 1.07),
    ("CF-Nylon (3D print)", 1.18),
];

/// How a physical material looks (sRGB); the default material has no colour of its own.
pub fn material_color(name: &str) -> Option<[u8; 3]> {
    Some(match name {
        "Steel" => [158, 164, 172],
        "Stainless Steel" => [186, 191, 198],
        "Aluminum" => [204, 208, 214],
        "Brass" => [208, 172, 88],
        "Copper" => [204, 122, 80],
        "Titanium" => [150, 148, 158],
        "Cast Iron" => [104, 104, 110],
        "ABS Plastic" => [222, 222, 216],
        "PLA" => [110, 160, 222],
        "Nylon" => [236, 230, 214],
        "Polycarbonate" => [196, 216, 232],
        "Oak" => [190, 146, 92],
        "Glass" => [170, 208, 218],
        "PLA (3D print)" => [230, 185, 60],
        "PETG (3D print)" => [120, 190, 210],
        "ABS (3D print)" => [214, 70, 60],
        "ASA (3D print)" => [235, 235, 228],
        "CF-Nylon (3D print)" => [52, 52, 56],
        _ => return None,
    })
}

use macroquad::color::{BROWN, Color, GRAY, GREEN, MAGENTA};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Voxel {
    id: u16,
}

impl Voxel {
    pub const AIR: Voxel = Voxel { id: 0 };
    pub const DIRT: Voxel = Voxel { id: 1 };
    pub const GRASS: Voxel = Voxel { id: 2 };
    pub const STONE: Voxel = Voxel { id: 3 };


    pub fn is_solid(self) -> bool {
        self != Voxel::AIR
    }

    pub fn color(self) -> Color {
        match self {
            Voxel::DIRT => BROWN,
            Voxel::GRASS => GREEN,
            Voxel::STONE => GRAY,
            _ => MAGENTA, // unknown voxel
        }
    }
}

impl Default for Voxel {
    fn default() -> Self {
        Voxel::AIR
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_air_is_not_solid() {
        assert!(!Voxel::AIR.is_solid());
        assert!(Voxel::DIRT.is_solid() && Voxel::GRASS.is_solid() && Voxel::STONE.is_solid());
    }

    #[test]
    fn default_is_air() {
        assert_eq!(Voxel::default(), Voxel::AIR);
    }

    #[test]
    fn voxel_is_as_small_as_the_old_u16_id() {
        assert_eq!(std::mem::size_of::<Voxel>(), 2);
    }
}
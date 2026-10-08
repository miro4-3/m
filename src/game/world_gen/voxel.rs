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

    /// Number of voxel types, air included. Every `kind()` is below this.
    pub const KINDS: usize = 4;

    /// Dense index of this voxel type, usable as an array index.
    pub fn kind(self) -> usize {
        self.id as usize
    }

    pub fn from_kind(kind: usize) -> Voxel {
        Voxel { id: kind as u16 }
    }

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
    fn kinds_are_dense_and_round_trip() {
        let all = [Voxel::AIR, Voxel::DIRT, Voxel::GRASS, Voxel::STONE];
        let mut kinds: Vec<_> = all.iter().map(|v| v.kind()).collect();
        kinds.sort();
        assert_eq!(kinds, (0..Voxel::KINDS).collect::<Vec<_>>());
        assert!(all.iter().all(|&v| Voxel::from_kind(v.kind()) == v));
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
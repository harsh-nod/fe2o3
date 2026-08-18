/// Safe host-compilable surface used only by semantic source fixtures.
///
/// The methods preserve operations that a future authenticated MIR importer
/// must recognize. They grant no device or proof authority.
pub struct KernelContext<'a> {
    pub a: &'a [u16],
    pub b: &'a [u16],
    pub c: &'a mut [f32],
    pub lds: &'a mut [Option<(u32, u16)>],
    pub lane: usize,
    pub group_x: usize,
    pub group_y: usize,
    pub m: usize,
    pub n: usize,
    pub k: usize,
    pub lda: usize,
    pub ldb: usize,
    pub ldc: usize,
    pub phase: usize,
    pub alpha: f32,
    pub beta: f32,
}

impl KernelContext<'_> {
    pub fn row(&self) -> usize {
        self.group_y * 16 + self.lane % 16
    }

    pub fn column(&self) -> usize {
        self.group_x * 16 + self.lane % 16
    }

    pub fn depth(&self) -> usize {
        self.phase * 16 + 4 * (self.lane / 16)
    }

    pub fn load_a(&self, row: usize, depth: usize) -> u16 {
        self.a[row * self.lda + depth]
    }

    pub fn load_b(&self, depth: usize, column: usize) -> u16 {
        self.b[depth * self.ldb + column]
    }

    pub fn load_c(&self, row: usize, column: usize) -> f32 {
        self.c[row * self.ldc + column]
    }

    pub fn store_c(&mut self, row: usize, column: usize, value: f32) {
        self.c[row * self.ldc + column] = value;
    }

    pub fn store_c_index(&mut self, index: usize, value: f32) {
        self.c[index] = value;
    }

    pub fn stage(&mut self, slot: usize, epoch: u32, value: u16) {
        self.lds[slot] = Some((epoch, value));
    }

    pub fn read_stage(&self, slot: usize, epoch: u32) -> u16 {
        match self.lds[slot] {
            Some((actual_epoch, value)) if actual_epoch == epoch => value,
            _ => 0,
        }
    }

    pub fn publish_barrier(&self) {}

    pub fn reuse_barrier(&self) {}

    pub fn begin_async_stage(&mut self, slot: usize, epoch: u32, value: u16) {
        self.stage(slot, epoch, value);
    }

    pub fn wait_stage(&self) {}

    pub fn mfma_fragment(
        &self,
        lhs: [u16; 4],
        rhs: [u16; 4],
        mut accumulator: [f32; 4],
    ) -> [f32; 4] {
        for component in 0..4 {
            let left = f32::from_bits(u32::from(lhs[component]) << 16);
            let right = f32::from_bits(u32::from(rhs[component]) << 16);
            accumulator[component] += left * right;
        }
        accumulator
    }
}

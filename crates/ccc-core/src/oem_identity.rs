//! 原厂将 PCI 型号低两字节交换后存入 page0[2..6]。
pub fn product_id(saved: [u8; 4]) -> u32 {
    u32::from_le_bytes([saved[1], saved[0], saved[2], saved[3]])
}

#[cfg(test)]
mod tests {
    #[test]
    fn saved_product_id_restores_original_pci_order() {
        assert_eq!(super::product_id([0x50, 0xd8, 0, 0]), 0x50d8);
        assert_eq!(super::product_id([0x10, 0x0a, 0, 0]), 4106);
    }
}

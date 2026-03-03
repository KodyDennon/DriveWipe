mod common;

use drivewipe_core::partition::PartitionTable;
use drivewipe_core::partition::gpt::GptTable;
use drivewipe_core::partition::mbr::MbrTable;
use drivewipe_core::partition::ops;

use common::MockDevice;

#[test]
fn test_gpt_create_partition() {
    let mut device = MockDevice::new(1024 * 1024);
    let mut table = PartitionTable::Gpt(GptTable {
        disk_guid: "TEST-GUID".to_string(),
        first_usable_lba: 34,
        last_usable_lba: 2047,
        entry_count: 128,
        entry_size: 128,
        partitions: vec![],
    });

    let result = ops::create_partition(&mut device, &mut table, 100, 500, "EFI-TYPE", "TestPart");
    assert!(result.is_ok());

    let part = result.unwrap();
    assert_eq!(part.start_lba, 100);
    assert_eq!(part.end_lba, 500);
    assert_eq!(part.name, "TestPart");

    let partitions = table.partitions();
    assert_eq!(partitions.len(), 1);
}

#[test]
fn test_gpt_create_partition_overlap_rejected() {
    let mut device = MockDevice::new(1024 * 1024);
    let mut table = PartitionTable::Gpt(GptTable {
        disk_guid: "TEST-GUID".to_string(),
        first_usable_lba: 34,
        last_usable_lba: 2047,
        entry_count: 128,
        entry_size: 128,
        partitions: vec![],
    });

    // Create first partition
    ops::create_partition(&mut device, &mut table, 100, 500, "TYPE1", "Part1").unwrap();

    // Overlapping partition should fail
    let result = ops::create_partition(&mut device, &mut table, 400, 600, "TYPE2", "Part2");
    assert!(result.is_err());
}

#[test]
fn test_gpt_delete_partition() {
    let mut device = MockDevice::new(1024 * 1024);
    let mut table = PartitionTable::Gpt(GptTable {
        disk_guid: "TEST-GUID".to_string(),
        first_usable_lba: 34,
        last_usable_lba: 2047,
        entry_count: 128,
        entry_size: 128,
        partitions: vec![],
    });

    ops::create_partition(&mut device, &mut table, 100, 500, "TYPE", "Part").unwrap();
    assert_eq!(table.partitions().len(), 1);

    ops::delete_partition(&mut device, &mut table, 0).unwrap();
    assert_eq!(table.partitions().len(), 0);
}

#[test]
fn test_gpt_resize_partition() {
    let mut device = MockDevice::new(1024 * 1024);
    let mut table = PartitionTable::Gpt(GptTable {
        disk_guid: "TEST-GUID".to_string(),
        first_usable_lba: 34,
        last_usable_lba: 2047,
        entry_count: 128,
        entry_size: 128,
        partitions: vec![],
    });

    ops::create_partition(&mut device, &mut table, 100, 500, "TYPE", "Part").unwrap();

    // Grow the partition
    ops::resize_partition(&mut device, &mut table, 0, 800).unwrap();
    let parts = table.partitions();
    assert_eq!(parts[0].end_lba, 800);
    assert_eq!(parts[0].size_bytes, (800 - 100 + 1) * 512);
}

#[test]
fn test_gpt_move_partition() {
    let mut device = MockDevice::new(1024 * 1024);
    let mut table = PartitionTable::Gpt(GptTable {
        disk_guid: "TEST-GUID".to_string(),
        first_usable_lba: 34,
        last_usable_lba: 2047,
        entry_count: 128,
        entry_size: 128,
        partitions: vec![],
    });

    ops::create_partition(&mut device, &mut table, 100, 500, "TYPE", "Part").unwrap();

    // Move the partition (preserves size)
    ops::move_partition(&mut device, &mut table, 0, 600).unwrap();
    let parts = table.partitions();
    assert_eq!(parts[0].start_lba, 600);
    assert_eq!(parts[0].end_lba, 1000); // 600 + (500 - 100) = 1000
}

#[test]
fn test_mbr_create_max_4_partitions() {
    let mut device = MockDevice::new(1024 * 1024);
    let mut table = PartitionTable::Mbr(MbrTable {
        disk_signature: 0x12345678,
        partitions: vec![],
    });

    for i in 0..4 {
        let start = 100 + i * 200;
        let end = start + 100;
        ops::create_partition(
            &mut device,
            &mut table,
            start,
            end,
            "0x83",
            &format!("P{}", i + 1),
        )
        .unwrap();
    }

    // 5th should fail
    let result = ops::create_partition(&mut device, &mut table, 1000, 1100, "0x83", "P5");
    assert!(result.is_err());
}

#[test]
fn test_preview_operation() {
    let preview = ops::preview_operation("Delete partition 2", &[2], true);
    assert_eq!(preview.description, "Delete partition 2");
    assert!(preview.data_loss_risk);
    assert_eq!(preview.affected_partitions, vec![2]);
}

#[test]
fn test_gpt_crc32_validation_empty_data() {
    let table = GptTable {
        disk_guid: "TEST".to_string(),
        first_usable_lba: 34,
        last_usable_lba: 2047,
        entry_count: 128,
        entry_size: 128,
        partitions: vec![],
    };
    // Empty/too-small data should fail validation
    assert!(!table.validate_crc(&[0u8; 100]));
}

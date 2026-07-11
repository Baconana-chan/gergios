/* tests/bt_ffi/test_bt_ipc.cpp
 *
 * Phase 9.1: Bluetooth IPC message encoding tests.
 *
 * Tests the MINIX IPC message encoding format used by libbluetooth:
 *   — msg_write_i32 / msg_read_i32: 32-bit LE at payload offsets
 *   — msg_pack_bdaddr: BD_ADDR (6 bytes) into offsets 8 + 16
 *   — msg_write_name: null-terminated string at offset 32 (max 48 bytes)
 *
 * Encoding verified against minix/lib/libbluetooth/bluetooth.c
 * and minix/include/minix/com.h (BT message field definitions).
 */

#include <catch.hpp>
#include <cstring>
#include <cstdint>

/* =================================================================
 * Message model
 *
 * MINIX IPC message is 64 bytes total:
 *   Bytes 0-3:   m_source  (endpoint_t, int32)
 *   Bytes 4-7:   m_type    (int32)
 *   Bytes 8-63:  payload   (56 bytes)
 *
 * Payload layout (mess_4 union):
 *   Offset  0 (byte 8):  m4_l1 (int32)
 *   Offset  8 (byte 16): m4_l2 (int32)
 *   Offset 16 (byte 24): m4_l3 (int32)
 *   Offset 24 (byte 32): m4_l4 (int32)
 *   Offset 32 (byte 40): name / extra data (24 bytes)
 * ================================================================= */

static constexpr size_t MESSAGE_SIZE = 64;
static constexpr size_t PAYLOAD_OFFSET = 8;

/** Get pointer to the payload area of a message buffer. */
static uint8_t *msg_payload(uint8_t *m) {
    return m + PAYLOAD_OFFSET;
}

/** Write an int32 at the given payload offset (LE). */
static void msg_write_i32(uint8_t *m, int offset, int32_t val) {
    uint8_t *p = msg_payload(m) + offset;
    p[0] = static_cast<uint8_t>(val & 0xFF);
    p[1] = static_cast<uint8_t>((val >> 8) & 0xFF);
    p[2] = static_cast<uint8_t>((val >> 16) & 0xFF);
    p[3] = static_cast<uint8_t>((val >> 24) & 0xFF);
}

/** Read an int32 from the given payload offset (LE). */
static int32_t msg_read_i32(const uint8_t *m, int offset) {
    const uint8_t *p = (m + PAYLOAD_OFFSET) + offset;
    return static_cast<int32_t>(
        static_cast<uint32_t>(p[0])
        | (static_cast<uint32_t>(p[1]) << 8)
        | (static_cast<uint32_t>(p[2]) << 16)
        | (static_cast<uint32_t>(p[3]) << 24)
    );
}

/** Pack a BD_ADDR (6 bytes) into payload offsets 8 (low 32 bits) and 16 (high 16 bits). */
static void msg_pack_bdaddr(uint8_t *m, const uint8_t bdaddr[6]) {
    uint32_t low = static_cast<uint32_t>(bdaddr[0])
                 | (static_cast<uint32_t>(bdaddr[1]) << 8)
                 | (static_cast<uint32_t>(bdaddr[2]) << 16)
                 | (static_cast<uint32_t>(bdaddr[3]) << 24);
    uint16_t high = static_cast<uint16_t>(bdaddr[4])
                  | (static_cast<uint16_t>(bdaddr[5]) << 8);

    msg_write_i32(m, 8, static_cast<int32_t>(low));
    msg_write_i32(m, 16, static_cast<int32_t>(high));
}

/** Extract BD_ADDR from payload offsets 8 and 16. */
static void msg_unpack_bdaddr(const uint8_t *m, uint8_t bdaddr[6]) {
    uint32_t low = static_cast<uint32_t>(msg_read_i32(m, 8));
    uint16_t high = static_cast<uint16_t>(msg_read_i32(m, 16) & 0xFFFF);

    bdaddr[0] = static_cast<uint8_t>(low & 0xFF);
    bdaddr[1] = static_cast<uint8_t>((low >> 8) & 0xFF);
    bdaddr[2] = static_cast<uint8_t>((low >> 16) & 0xFF);
    bdaddr[3] = static_cast<uint8_t>((low >> 24) & 0xFF);
    bdaddr[4] = static_cast<uint8_t>(high & 0xFF);
    bdaddr[5] = static_cast<uint8_t>((high >> 8) & 0xFF);
}

/** Copy a name string into payload offset 32 (max 48 bytes including null). */
static void msg_write_name(uint8_t *m, const char *name, size_t maxlen) {
    uint8_t *dst = msg_payload(m) + 32;
    size_t len = strnlen(name, maxlen - 1);
    std::memcpy(dst, name, len);
    dst[len] = '\0';
}

/** Read a name string from payload offset 32 into a buffer. */
static void msg_read_name(const uint8_t *m, char *buf, size_t bufsize) {
    const uint8_t *src = (m + PAYLOAD_OFFSET) + 32;
    size_t len = strnlen(reinterpret_cast<const char *>(src), bufsize - 1);
    std::memcpy(buf, src, len);
    buf[len] = '\0';
}

/* =================================================================
 * Test helpers
 * ================================================================= */

/** Create a zeroed message buffer. */
static std::vector<uint8_t> make_message() {
    return std::vector<uint8_t>(MESSAGE_SIZE, 0);
}

/* =================================================================
 * Test Cases: msg_write_i32 / msg_read_i32
 * ================================================================= */

TEST_CASE("msg_write_i32 writes little-endian at payload offset 0",
          "[bt][ipc][write_i32]") {
    auto msg = make_message();
    msg_write_i32(msg.data(), 0, 0x12345678);

    // Offset 0 in payload = byte 8 in message
    REQUIRE(msg[8]  == 0x78);  // low byte
    REQUIRE(msg[9]  == 0x56);
    REQUIRE(msg[10] == 0x34);
    REQUIRE(msg[11] == 0x12);  // high byte
}

TEST_CASE("msg_write_i32 writes at payload offset 8 (m4_l2 position)",
          "[bt][ipc][write_i32]") {
    auto msg = make_message();
    msg_write_i32(msg.data(), 8, 0xAABBCCDD);

    // Offset 8 in payload = byte 16 in message
    REQUIRE(msg[16] == 0xDD);
    REQUIRE(msg[17] == 0xCC);
    REQUIRE(msg[18] == 0xBB);
    REQUIRE(msg[19] == 0xAA);
}

TEST_CASE("msg_write_i32 writes at payload offset 16 (m4_l3 position)",
          "[bt][ipc][write_i32]") {
    auto msg = make_message();
    msg_write_i32(msg.data(), 16, 0xDEADBEEF);

    REQUIRE(msg[24] == 0xEF);
    REQUIRE(msg[25] == 0xBE);
    REQUIRE(msg[26] == 0xAD);
    REQUIRE(msg[27] == 0xDE);
}

TEST_CASE("msg_write_i32 writes at payload offset 24 (m4_l4 position)",
          "[bt][ipc][write_i32]") {
    auto msg = make_message();
    msg_write_i32(msg.data(), 24, 0xCAFEBABE);

    REQUIRE(msg[32] == 0xBE);
    REQUIRE(msg[33] == 0xBA);
    REQUIRE(msg[34] == 0xFE);
    REQUIRE(msg[35] == 0xCA);
}

TEST_CASE("msg_write_i32 and msg_read_i32 round-trip correctly",
          "[bt][ipc][write_i32]") {
    auto msg = make_message();

    int32_t values[] = {
        0, 1, -1, 127, -128, 32767, -32768,
        0x7FFFFFFF,           // INT32_MAX
        static_cast<int32_t>(0x80000000),  // INT32_MIN
        0x12345678, 0xDEADBEEF
    };

    for (auto val : values) {
        msg_write_i32(msg.data(), 0, val);
        int32_t readback = msg_read_i32(msg.data(), 0);
        REQUIRE(readback == val);
    }
}

TEST_CASE("msg_write_i32 preserves other payload fields (no side effects)",
          "[bt][ipc][write_i32]") {
    auto msg = make_message();
    // Fill with a pattern
    for (size_t i = 0; i < MESSAGE_SIZE; i++)
        msg[i] = 0xFF;

    // Write at offset 8 only
    msg_write_i32(msg.data(), 8, 0);

    // Bytes outside the written range should be preserved
    REQUIRE(msg[8]  == 0x00);  // written
    REQUIRE(msg[9]  == 0x00);
    REQUIRE(msg[10] == 0x00);
    REQUIRE(msg[11] == 0x00);
    REQUIRE(msg[12] == 0xFF);  // not written
    REQUIRE(msg[15] == 0xFF);  // not written
}

/* =================================================================
 * Test Cases: msg_pack_bdaddr
 * ================================================================= */

TEST_CASE("msg_pack_bdaddr packs standard BD_ADDR into offsets 8 and 16",
          "[bt][ipc][bdaddr]") {
    auto msg = make_message();

    // BD_ADDR: 01:02:03:04:05:06 (6 bytes, LSB-first on wire)
    uint8_t addr[6] = {0x01, 0x02, 0x03, 0x04, 0x05, 0x06};
    msg_pack_bdaddr(msg.data(), addr);

    // Low 32 bits (addr[0..3]) at payload offset 8 → message bytes 16-19
    // 0x04030201 in LE: 01, 02, 03, 04
    REQUIRE(msg[16] == 0x01);
    REQUIRE(msg[17] == 0x02);
    REQUIRE(msg[18] == 0x03);
    REQUIRE(msg[19] == 0x04);

    // High 16 bits (addr[4..5]) at payload offset 16 → message bytes 24-25
    // 0x0605 in LE: 05, 06
    REQUIRE(msg[24] == 0x05);
    REQUIRE(msg[25] == 0x06);
}

TEST_CASE("msg_pack_bdaddr and msg_unpack_bdaddr round-trip",
          "[bt][ipc][bdaddr]") {
    auto msg = make_message();

    uint8_t addrs[][6] = {
        {0x00, 0x00, 0x00, 0x00, 0x00, 0x00},
        {0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF},
        {0x01, 0x02, 0x03, 0x04, 0x05, 0x06},
        {0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56},
        {0x11, 0x22, 0x33, 0x44, 0x55, 0x66},
    };

    for (const auto &addr : addrs) {
        std::memset(msg.data(), 0, MESSAGE_SIZE);
        msg_pack_bdaddr(msg.data(), addr);

        uint8_t unpacked[6];
        msg_unpack_bdaddr(msg.data(), unpacked);

        for (int i = 0; i < 6; i++)
            REQUIRE(unpacked[i] == addr[i]);
    }
}

TEST_CASE("msg_pack_bdaddr with all-zero address",
          "[bt][ipc][bdaddr]") {
    auto msg = make_message();
    uint8_t addr[6] = {0, 0, 0, 0, 0, 0};
    msg_pack_bdaddr(msg.data(), addr);

    REQUIRE(msg_read_i32(msg.data(), 8) == 0);
    REQUIRE(msg_read_i32(msg.data(), 16) == 0);
}

TEST_CASE("msg_pack_bdaddr with all-0xFF address",
          "[bt][ipc][bdaddr]") {
    auto msg = make_message();
    uint8_t addr[6] = {0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF};
    msg_pack_bdaddr(msg.data(), addr);

    REQUIRE(msg_read_i32(msg.data(), 8) == static_cast<int32_t>(0xFFFFFFFF));
    REQUIRE((msg_read_i32(msg.data(), 16) & 0xFFFF) == 0xFFFF);
}

TEST_CASE("msg_pack_bdaddr does not clobber offset 0 (m4_l1, BT_REG_PSM)",
          "[bt][ipc][bdaddr]") {
    auto msg = make_message();

    // Simulate bt_register_service: write PSM at offset 0, then BD_ADDR at offset 8+16
    msg_write_i32(msg.data(), 0, 0x0003);  // PSM = RFCOMM
    uint8_t addr[6] = {0x01, 0x02, 0x03, 0x04, 0x05, 0x06};
    msg_pack_bdaddr(msg.data(), addr);

    // Offset 0 should still hold PSM
    REQUIRE(msg_read_i32(msg.data(), 0) == 0x0003);
}

/* =================================================================
 * Test Cases: msg_write_name
 * ================================================================= */

TEST_CASE("msg_write_name writes null-terminated string at payload offset 32",
          "[bt][ipc][name]") {
    auto msg = make_message();
    const char *name = "My Bluetooth Device";

    msg_write_name(msg.data(), name, 48);

    char readback[48] = {};
    msg_read_name(msg.data(), readback, sizeof(readback));
    REQUIRE(std::string(readback) == "My Bluetooth Device");
}

TEST_CASE("msg_write_name writes at message bytes 40-63 (payload offset 32-55)",
          "[bt][ipc][name]") {
    auto msg = make_message();
    const char *name = "ABCDEFGH";

    msg_write_name(msg.data(), name, 48);

    // Name starts at payload offset 32 = message byte 40
    REQUIRE(msg[40] == 'A');
    REQUIRE(msg[41] == 'B');
    REQUIRE(msg[47] == 'H');
    REQUIRE(msg[48] == '\0');  // null terminator at byte 48 (offset 32 + 8 = 40)
}

TEST_CASE("msg_write_name truncates at maxlen",
          "[bt][ipc][name]") {
    auto msg = make_message();
    const char *name = "This is a very long device name for testing";

    msg_write_name(msg.data(), name, 16);
    char readback[48] = {};
    msg_read_name(msg.data(), readback, sizeof(readback));

    // Should be truncated to maxlen-1 = 15 chars + null
    REQUIRE(std::string(readback) == "This is a very");
    REQUIRE(std::strlen(readback) == 15);
}

TEST_CASE("msg_write_name handles empty string",
          "[bt][ipc][name]") {
    auto msg = make_message();
    msg_write_name(msg.data(), "", 48);

    char readback[48] = {};
    msg_read_name(msg.data(), readback, sizeof(readback));
    REQUIRE(std::string(readback) == "");
    REQUIRE(msg[40] == '\0');  // null at offset 32 (payload) = byte 40 (message)
}

TEST_CASE("msg_write_name writes correct payload structure (SIM card test)",
          "[bt][ipc][name]") {
    // Simulate the full bt_set_name() message structure
    auto msg = make_message();

    // m_type = BT_RQ_SET_NAME (= BT_RQ_BASE + 7 = 0x1027)
    int32_t m_type = 0x1027;
    std::memcpy(&msg[4], &m_type, sizeof(m_type));

    // Name at offset 32
    const char *name = "SIM card";
    msg_write_name(msg.data(), name, 48);

    // Verify name position
    REQUIRE(msg[40] == 'S');  // 'S' at byte 40
    REQUIRE(msg[47] == '\0'); // null terminator after "SIM card" (8 chars)

    // Verify m_type is not clobbered
    int32_t type_check;
    std::memcpy(&type_check, &msg[4], sizeof(type_check));
    REQUIRE(type_check == 0x1027);

    // Verify all other fields are zero
    for (int i = 8; i < 40; i++)
        REQUIRE(msg[i] == 0);
}

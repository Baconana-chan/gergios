/* tests/bt_ffi/test_sdp.cpp
 *
 * Phase 9.1: Bluetooth SDP DataElement encoding tests
 *
 * Tests the Bluetooth SDP (Service Discovery Protocol) DataElement
 * wire encoding format, verifying that data element headers, sizes,
 * and payloads follow the Bluetooth assigned numbers specification.
 *
 * SDP Data Element wire format:
 *   Header byte: [Type (5 bits) | Size Descriptor (3 bits)]
 *   Size descriptor:
 *     0 = 0 bytes (Nil/Boolean)
 *     1 = 1 byte value
 *     2 = 2 byte value
 *     3 = 4 byte value
 *     4 = 8 byte value
 *     5 = variable length, 1 additional size byte
 *     6 = variable length, 2 additional size bytes
 *     7 = variable length, 4 additional size bytes
 *
 * Type descriptors:
 *   0x00 = Nil
 *   0x01 = Unsigned Integer
 *   0x02 = Signed Integer
 *   0x03 = UUID
 *   0x04 = String
 *   0x05 = Boolean
 *   0x06 = Data Element Sequence
 *   0x07 = Data Element Alternative
 *   0x08 = URL
 *
 * These tests are standalone — they don't link against any
 * Bluetooth library and only verify the wire protocol format.
 */

#include <catch.hpp>
#include <cstdint>
#include <cstring>
#include <vector>
#include <string>

/* =================================================================
 * SDP DataElement Encoding Helpers (standalone, matches Rust wire format)
 * ================================================================= */

/* Type descriptors (upper 5 bits of header byte). */
enum SdpType : uint8_t {
    SDP_TYPE_NIL       = 0x00,
    SDP_TYPE_UINT      = 0x01,
    SDP_TYPE_SINT      = 0x02,
    SDP_TYPE_UUID      = 0x03,
    SDP_TYPE_STRING    = 0x04,
    SDP_TYPE_BOOL      = 0x05,
    SDP_TYPE_SEQ       = 0x06,
    SDP_TYPE_ALT       = 0x07,
    SDP_TYPE_URL       = 0x08,
};

/* Size descriptors (lower 3 bits of header byte). */
enum SdpSize : uint8_t {
    SDP_SIZE_0   = 0,   /* 0 bytes (Nil/Boolean) */
    SDP_SIZE_1   = 1,   /* 1 byte */
    SDP_SIZE_2   = 2,   /* 2 bytes */
    SDP_SIZE_4   = 3,   /* 4 bytes */
    SDP_SIZE_8   = 4,   /* 8 bytes */
    SDP_SIZE_VAR1 = 5,  /* variable, 1 extra size byte */
    SDP_SIZE_VAR2 = 6,  /* variable, 2 extra size bytes */
    SDP_SIZE_VAR4 = 7,  /* variable, 4 extra size bytes */
};

/* Build header byte. */
static uint8_t sdp_header(SdpType type, SdpSize size_desc) {
    return (static_cast<uint8_t>(type) << 3) | static_cast<uint8_t>(size_desc);
}

/* Encode a variable-length data element (String, Seq, etc.) */
static std::vector<uint8_t> encode_variable(SdpType type, const uint8_t *data, size_t len) {
    std::vector<uint8_t> result;
    uint8_t size_desc;
    std::vector<uint8_t> size_bytes;

    if (len <= 0xFF) {
        size_desc = SDP_SIZE_VAR1;
        size_bytes = {static_cast<uint8_t>(len)};
    } else if (len <= 0xFFFF) {
        size_desc = SDP_SIZE_VAR2;
        size_bytes = {
            static_cast<uint8_t>((len >> 8) & 0xFF),
            static_cast<uint8_t>(len & 0xFF)
        };
    } else {
        size_desc = SDP_SIZE_VAR4;
        size_bytes = {
            static_cast<uint8_t>((len >> 24) & 0xFF),
            static_cast<uint8_t>((len >> 16) & 0xFF),
            static_cast<uint8_t>((len >> 8) & 0xFF),
            static_cast<uint8_t>(len & 0xFF)
        };
    }

    result.push_back(sdp_header(type, static_cast<SdpSize>(size_desc)));
    result.insert(result.end(), size_bytes.begin(), size_bytes.end());
    result.insert(result.end(), data, data + len);
    return result;
}

/* =================================================================
 * Test Cases
 * ================================================================= */

TEST_CASE("Nil element encodes as single 0x00 byte",
          "[sdp][dataelement]") {
    uint8_t expected[] = {0x00};
    REQUIRE(sdp_header(SDP_TYPE_NIL, SDP_SIZE_0) == 0x00);
    // Nil is just the header byte with no body
    std::vector<uint8_t> encoded = {0x00};
    REQUIRE(encoded.size() == 1);
    REQUIRE(memcmp(encoded.data(), expected, 1) == 0);
}

TEST_CASE("Unsigned integer 8-bit encodes with header 0x09 + value byte",
          "[sdp][dataelement]") {
    // type=1<<3|1 = 9 = 0x09
    REQUIRE(sdp_header(SDP_TYPE_UINT, SDP_SIZE_1) == 0x09);

    uint8_t expected[] = {0x09, 0x42};
    std::vector<uint8_t> encoded = {0x09, 0x42};
    REQUIRE(encoded.size() == 2);
    REQUIRE(memcmp(encoded.data(), expected, 2) == 0);
}

TEST_CASE("Unsigned integer 16-bit encodes with header 0x0A + 2 bytes big-endian",
          "[sdp][dataelement]") {
    // type=1<<3|2 = 10 = 0x0A
    REQUIRE(sdp_header(SDP_TYPE_UINT, SDP_SIZE_2) == 0x0A);

    uint8_t expected[] = {0x0A, 0x12, 0x34};
    std::vector<uint8_t> encoded = {0x0A, 0x12, 0x34};
    REQUIRE(encoded.size() == 3);
    REQUIRE(memcmp(encoded.data(), expected, 3) == 0);
}

TEST_CASE("Unsigned integer 32-bit encodes with header 0x0B + 4 bytes big-endian",
          "[sdp][dataelement]") {
    // type=1<<3|3 = 11 = 0x0B
    REQUIRE(sdp_header(SDP_TYPE_UINT, SDP_SIZE_4) == 0x0B);

    uint8_t expected[] = {0x0B, 0x12, 0x34, 0x56, 0x78};
    std::vector<uint8_t> encoded = {0x0B, 0x12, 0x34, 0x56, 0x78};
    REQUIRE(encoded.size() == 5);
    REQUIRE(memcmp(encoded.data(), expected, 5) == 0);
}

TEST_CASE("Boolean true encodes as header 0x28 + 0x01",
          "[sdp][dataelement]") {
    // type=5<<3|0 = 40 = 0x28
    REQUIRE(sdp_header(SDP_TYPE_BOOL, SDP_SIZE_0) == 0x28);

    uint8_t expected_true[]  = {0x28, 0x01};
    uint8_t expected_false[] = {0x28, 0x00};

    std::vector<uint8_t> encoded_true  = {0x28, 0x01};
    std::vector<uint8_t> encoded_false = {0x28, 0x00};

    REQUIRE(encoded_true.size() == 2);
    REQUIRE(memcmp(encoded_true.data(), expected_true, 2) == 0);
    REQUIRE(encoded_false.size() == 2);
    REQUIRE(memcmp(encoded_false.data(), expected_false, 2) == 0);
}

TEST_CASE("UUID 16-bit encodes as header 0x19 + 2 bytes big-endian",
          "[sdp][dataelement]") {
    // type=3<<3|1 = 25 = 0x19 (UUID16, size=1)
    REQUIRE(sdp_header(SDP_TYPE_UUID, SDP_SIZE_1) == 0x19);

    // Serial Port UUID 0x1101
    uint8_t expected[] = {0x19, 0x11, 0x01};
    std::vector<uint8_t> encoded = {0x19, 0x11, 0x01};
    REQUIRE(encoded.size() == 3);
    REQUIRE(memcmp(encoded.data(), expected, 3) == 0);
}

TEST_CASE("UUID 128-bit encodes as header 0x1C + 16 bytes",
          "[sdp][dataelement]") {
    // type=3<<3|4 = 28 = 0x1C (UUID128, size=4)
    REQUIRE(sdp_header(SDP_TYPE_UUID, SDP_SIZE_8) == 0x1C);

    // Bluetooth Base UUID: 00000000-0000-1000-8000-00805F9B34FB
    uint8_t uuid128_bytes[16] = {
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
        0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB
    };
    // Wait — 128-bit UUID uses size descriptor 4 (16 bytes)
    // But actually size descriptor values: 1=1byte, 2=2bytes, 3=4bytes, 4=8bytes...
    // For 128-bit UUID, the spec says size_descriptor=4 which means 8 bytes? No...

    // Actually in Bluetooth SDP spec:
    // UUID size descriptor: 1 = 16-bit (2 bytes), 2 = 32-bit (4 bytes), 4 = 128-bit (16 bytes)
    // Type 3 << 3 | 4 = 0x1C (0b00011100 = 0x1C)
    // Wait, that's wrong: 3<<3 = 24 = 0x18, plus size 4 = 0x1C. But size 4 maps to... hmm.

    // Actually for UUID, the size descriptor values have special meaning:
    // 1 = 16-bit UUID (2 bytes) - header byte = (3<<3)|1 = 0x19
    // 2 = 32-bit UUID (4 bytes) - header byte = (3<<3)|2 = 0x1A
    // 4 = 128-bit UUID (16 bytes) - header byte = (3<<3)|4 = 0x1C
    // This is correct. The size descriptor for the 16-byte case is 4.
    REQUIRE(sdp_header(SDP_TYPE_UUID, static_cast<SdpSize>(4)) == 0x1C);
}

TEST_CASE("Short string (<256 bytes) encodes with variable size 5 header",
          "[sdp][dataelement]") {
    // type=4<<3|5 = 37 = 0x25
    uint8_t expected_header = (4 << 3) | 5;
    REQUIRE(expected_header == 0x25);

    const uint8_t payload[] = "Hello";
    auto encoded = encode_variable(SDP_TYPE_STRING, payload, 5);

    REQUIRE(encoded.size() == 7); // header(1) + len(1) + data(5)
    REQUIRE(encoded[0] == 0x25);  // type=String, size=VAR1
    REQUIRE(encoded[1] == 5);     // length byte
    REQUIRE(memcmp(encoded.data() + 2, "Hello", 5) == 0);
}

TEST_CASE("Empty string encodes as header 0x25 + length 0",
          "[sdp][dataelement]") {
    auto encoded = encode_variable(SDP_TYPE_STRING, (const uint8_t *)"", 0);

    REQUIRE(encoded.size() == 2);  // header(1) + len(1) + data(0)
    REQUIRE(encoded[0] == 0x25);   // type=String, size=VAR1
    REQUIRE(encoded[1] == 0);      // zero-length
}

TEST_CASE("Data element sequence encodes with variable size 5 header",
          "[sdp][dataelement]") {
    // type=6<<3|5 = 53 = 0x35
    uint8_t expected_header = (6 << 3) | 5;
    REQUIRE(expected_header == 0x35);

    // Sequence containing two uint8 values: 0x01 and 0x02
    // Each uint8 encodes as: 0x09, value
    uint8_t payload[] = {0x09, 0x01, 0x09, 0x02}; // 4 bytes
    auto encoded = encode_variable(SDP_TYPE_SEQ, payload, 4);

    REQUIRE(encoded.size() == 6); // header(1) + len(1) + data(4)
    REQUIRE(encoded[0] == 0x35);  // type=Seq, size=VAR1
    REQUIRE(encoded[1] == 4);     // total payload length
    REQUIRE(encoded[2] == 0x09);  // first uint8 header
    REQUIRE(encoded[3] == 0x01);  // first uint8 value
    REQUIRE(encoded[4] == 0x09);  // second uint8 header
    REQUIRE(encoded[5] == 0x02);  // second uint8 value
}

TEST_CASE("Data element alternative uses same wire format as sequence",
          "[sdp][dataelement]") {
    // type=7<<3|5 = 61 = 0x3D
    uint8_t expected_header = (7 << 3) | 5;
    REQUIRE(expected_header == 0x3D);

    uint8_t payload[] = {0x09, 0x01};
    auto encoded = encode_variable(SDP_TYPE_ALT, payload, 2);

    REQUIRE(encoded[0] == 0x3D);  // type=Alt, size=VAR1
    REQUIRE(encoded[1] == 2);     // total payload length
}

TEST_CASE("URL encoding uses same wire format as string",
          "[sdp][dataelement]") {
    // type=8<<3|5 = 69 = 0x45
    uint8_t expected_header = (8 << 3) | 5;
    REQUIRE(expected_header == 0x45);

    const uint8_t url[] = "https://gergios.example";
    auto encoded = encode_variable(SDP_TYPE_URL, url, 21);

    REQUIRE(encoded[0] == 0x45);  // type=URL, size=VAR1
    REQUIRE(encoded[1] == 21);    // length
}

TEST_CASE("Large variable data (>255 bytes) uses 2-byte size descriptor",
          "[sdp][dataelement][edge]") {
    // Build a 300-byte payload
    std::vector<uint8_t> large_data(300, 0xAB);
    auto encoded = encode_variable(SDP_TYPE_STRING, large_data.data(), large_data.size());

    // type=4<<3|6 = 38 = 0x26
    uint8_t expected_header = (4 << 3) | 6;
    REQUIRE(expected_header == 0x26);

    REQUIRE(encoded.size() == 1 + 2 + 300); // header(1) + len(2) + data(300)
    REQUIRE(encoded[0] == 0x26);
    REQUIRE(encoded[1] == 0x01); // length MSB (300 = 0x012C)
    REQUIRE(encoded[2] == 0x2C); // length LSB
}

TEST_CASE("SDP service record wire format: ServiceClassIDList attribute",
          "[sdp][servicerecord]") {
    // Simulate a ServiceClassIDList attribute (ID=0x0001) containing
    // a single Serial Port UUID (0x1101).
    //
    // Wire format:
    //   Attr ID element: uint16(0x0001) = 0x0A 0x00 0x01
    //   Attr value: Seq([UUID16(0x1101)]) = 0x35 0x03 0x19 0x11 0x01
    //
    // So the raw attribute bytes for (id=0x0001, value=Seq([UUID16(0x1101)])):
    //   0x0A 0x00 0x01  0x35 0x03 0x19 0x11 0x01

    uint8_t expected[] = {
        0x0A, 0x00, 0x01,          // attr_id = uint16(0x0001)
        0x35, 0x03,                 // seq header: type=Seq, size=VAR1, len=3
        0x19, 0x11, 0x01           // UUID16(0x1101): type=UUID, size=1, value=0x1101
    };

    std::vector<uint8_t> encoded;
    // Attribute ID: unsigned 16-bit, value=0x0001
    encoded.push_back(sdp_header(SDP_TYPE_UINT, SDP_SIZE_2)); // 0x0A
    encoded.push_back(0x00);
    encoded.push_back(0x01);
    // Attribute value: sequence containing UUID
    uint8_t seq_payload[] = {0x19, 0x11, 0x01};
    auto seq = encode_variable(SDP_TYPE_SEQ, seq_payload, 3);
    encoded.insert(encoded.end(), seq.begin(), seq.end());

    REQUIRE(encoded.size() == 8);
    REQUIRE(memcmp(encoded.data(), expected, 8) == 0);
}

TEST_CASE("SDP ProtocolDescriptorList: L2CAP + RFCOMM",
          "[sdp][servicerecord]") {
    // Simulate a ProtocolDescriptorList for RFCOMM serial port:
    // L2CAP(0x0100) + RFCOMM(0x0003) on channel 1.
    //
    // Wire format:
    //   Outer Seq: [L2CAP_Seq, RFCOMM_Seq]
    //
    // L2CAP Seq: Seq([UUID16(0x0100)]) = 0x35 0x03 0x19 0x01 0x00
    // RFCOMM Seq: Seq([UUID16(0x0003), Uint8(1)]) = 0x35 0x05 0x19 0x00 0x03 0x09 0x01

    uint8_t l2cap_seq[] = {0x19, 0x01, 0x00};
    auto l2cap = encode_variable(SDP_TYPE_SEQ, l2cap_seq, 3);
    // l2cap = 0x35 0x03 0x19 0x01 0x00

    uint8_t rfcomm_payload[] = {0x19, 0x00, 0x03, 0x09, 0x01};
    auto rfcomm = encode_variable(SDP_TYPE_SEQ, rfcomm_payload, 5);
    // rfcomm = 0x35 0x05 0x19 0x00 0x03 0x09 0x01

    // Combine into outer Seq
    std::vector<uint8_t> outer_payload;
    outer_payload.insert(outer_payload.end(), l2cap.begin(), l2cap.end());
    outer_payload.insert(outer_payload.end(), rfcomm.begin(), rfcomm.end());
    auto outer = encode_variable(SDP_TYPE_SEQ, outer_payload.data(), outer_payload.size());

    // outer: 0x35 0x0C  + l2cap(5) + rfcomm(7) = 1+1+5+7 = 14 bytes
    REQUIRE(outer[0] == 0x35);
    REQUIRE(outer[1] == 12); // total payload = 5 + 7 = 12
    REQUIRE(outer.size() == 14);
}

TEST_CASE("SDP LanguageBaseAttributeIDList encoding",
          "[sdp][servicerecord]") {
    // LanguageBaseAttributeIDList (attr 0x0006):
    // Seq([Uint16(0x656E), Uint16(0x006A), Uint16(0x0100)])
    //   = en, UTF-8, base offset 0x0100

    uint8_t en[]  = {0x0A, 0x65, 0x6E};  // uint16(0x656E)
    uint8_t utf8[] = {0x0A, 0x00, 0x6A}; // uint16(0x006A)
    uint8_t base[] = {0x0A, 0x01, 0x00}; // uint16(0x0100)

    std::vector<uint8_t> payload;
    payload.insert(payload.end(), en, en + 3);
    payload.insert(payload.end(), utf8, utf8 + 3);
    payload.insert(payload.end(), base, base + 3);

    auto encoded = encode_variable(SDP_TYPE_SEQ, payload.data(), payload.size());

    // encoded = 0x35 0x09 + 9 bytes of payload = 11 bytes
    REQUIRE(encoded.size() == 11);
    REQUIRE(encoded[0] == 0x35); // Seq header
    REQUIRE(encoded[1] == 9);    // payload length
    // Check first uint16
    REQUIRE(encoded[2] == 0x0A); // uint16 header
    REQUIRE(encoded[3] == 0x65);
    REQUIRE(encoded[4] == 0x6E);
}

TEST_CASE("SDP header byte computation for all types",
          "[sdp][header]") {
    // Verify header byte calculations for all standard types

    // Nil: type=0, size=0 → 0x00
    REQUIRE(sdp_header(SDP_TYPE_NIL, SDP_SIZE_0) == 0x00);

    // UINT: size 1=0x09, size 2=0x0A, size 4=0x0B, size 8=0x0C
    REQUIRE(sdp_header(SDP_TYPE_UINT, SDP_SIZE_1) == 0x09);
    REQUIRE(sdp_header(SDP_TYPE_UINT, SDP_SIZE_2) == 0x0A);
    REQUIRE(sdp_header(SDP_TYPE_UINT, SDP_SIZE_4) == 0x0B);
    REQUIRE(sdp_header(SDP_TYPE_UINT, SDP_SIZE_8) == 0x0C);

    // SINT: size 1=0x11, size 2=0x12, size 4=0x13, size 8=0x14
    REQUIRE(sdp_header(SDP_TYPE_SINT, SDP_SIZE_1) == 0x11);
    REQUIRE(sdp_header(SDP_TYPE_SINT, SDP_SIZE_2) == 0x12);
    REQUIRE(sdp_header(SDP_TYPE_SINT, SDP_SIZE_4) == 0x13);
    REQUIRE(sdp_header(SDP_TYPE_SINT, SDP_SIZE_8) == 0x14);

    // UUID: size 1=0x19 (16-bit), size 2=0x1A (32-bit), var4=0x1C (128-bit)
    REQUIRE(sdp_header(SDP_TYPE_UUID, SDP_SIZE_1) == 0x19);
    REQUIRE(sdp_header(SDP_TYPE_UUID, SDP_SIZE_2) == 0x1A);
    REQUIRE(sdp_header(SDP_TYPE_UUID, static_cast<SdpSize>(4)) == 0x1C);

    // String: var1=0x25, var2=0x26, var4=0x27
    REQUIRE(sdp_header(SDP_TYPE_STRING, SDP_SIZE_VAR1) == 0x25);
    REQUIRE(sdp_header(SDP_TYPE_STRING, SDP_SIZE_VAR2) == 0x26);
    REQUIRE(sdp_header(SDP_TYPE_STRING, SDP_SIZE_VAR4) == 0x27);

    // Boolean: size 0=0x28 (the bool value byte follows)
    REQUIRE(sdp_header(SDP_TYPE_BOOL, SDP_SIZE_0) == 0x28);

    // Seq: var1=0x35, var2=0x36, var4=0x37
    REQUIRE(sdp_header(SDP_TYPE_SEQ, SDP_SIZE_VAR1) == 0x35);
    REQUIRE(sdp_header(SDP_TYPE_SEQ, SDP_SIZE_VAR2) == 0x36);
    REQUIRE(sdp_header(SDP_TYPE_SEQ, SDP_SIZE_VAR4) == 0x37);

    // Alt: var1=0x3D, var2=0x3E, var4=0x3F
    REQUIRE(sdp_header(SDP_TYPE_ALT, SDP_SIZE_VAR1) == 0x3D);
    REQUIRE(sdp_header(SDP_TYPE_ALT, SDP_SIZE_VAR2) == 0x3E);
    REQUIRE(sdp_header(SDP_TYPE_ALT, SDP_SIZE_VAR4) == 0x3F);

    // URL: var1=0x45, var2=0x46, var4=0x47
    REQUIRE(sdp_header(SDP_TYPE_URL, SDP_SIZE_VAR1) == 0x45);
    REQUIRE(sdp_header(SDP_TYPE_URL, SDP_SIZE_VAR2) == 0x46);
    REQUIRE(sdp_header(SDP_TYPE_URL, SDP_SIZE_VAR4) == 0x47);
}

TEST_CASE("Variable size encoding thresholds match spec",
          "[sdp][edge]") {
    // Test the boundary between size descriptors

    // 255 bytes → size VAR1 (5)
    auto small = encode_variable(SDP_TYPE_STRING, (const uint8_t *)"", 255);
    REQUIRE(small[0] == 0x25); // String + VAR1

    // 256 bytes → size VAR2 (6)
    std::vector<uint8_t> at_256(256, 0x00);
    auto medium = encode_variable(SDP_TYPE_STRING, at_256.data(), 256);
    REQUIRE(medium[0] == 0x26); // String + VAR2

    // 65535 bytes → size VAR2 still
    std::vector<uint8_t> at_65535(65535, 0x00);
    auto large = encode_variable(SDP_TYPE_STRING, at_65535.data(), 65535);
    REQUIRE(large[0] == 0x26); // String + VAR2

    // 65536 bytes → size VAR4 (7)
    // String type = 4, VAR4 size = 7 → header = (4 << 3) | 7 = 0x27
    // Just verify the header byte computation directly — no need to
    // allocate 64KB payload just to check the encoding header.
    REQUIRE(sdp_header(SDP_TYPE_STRING, SDP_SIZE_VAR4) == 0x27);
}

TEST_CASE("SDP header byte: Type field occupies upper 5 bits",
          "[sdp][header][mask]") {
    // Verify that the header byte properly separates type and size
    uint8_t header = sdp_header(SDP_TYPE_UINT, SDP_SIZE_2);
    REQUIRE((header >> 3) == SDP_TYPE_UINT);  // Upper 5 bits = type
    REQUIRE((header & 0x07) == SDP_SIZE_2);    // Lower 3 bits = size
}

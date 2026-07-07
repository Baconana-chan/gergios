/* C shim — minimal entry point for the Rust e1000 driver.
 *
 * The entire e1000 driver logic is implemented in Rust at
 * rust/e1000/.  This file is only the C entry point that the
 * MINIX process manager calls.
 *
 * The Rust library exports e1000_rust_main(), which calls
 * netdriver_task() with a Rust-implemented netdriver table.
 * All hardware access, packet send/receive, interrupts, and
 * statistics are handled in safe Rust code (with FFI to MINIX
 * C APIs for PCI, IRQ, and memory management).
 */

extern int e1000_rust_main(int argc, char *argv[]);

int
main(int argc, char *argv[])
{
	return e1000_rust_main(argc, argv);
}

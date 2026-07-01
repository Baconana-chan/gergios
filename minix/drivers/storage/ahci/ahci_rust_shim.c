/* ahci_rust_shim.c — C shim for Rust AHCI driver (minix-ahci staticlib)
 *
 * This file replaces the original C main() in ahci.c with a thin shim
 * that delegates to the Rust implementation (ahci_rust_main).
 *
 * The Rust static library (libminix_ahci.a) provides the full AHCI driver:
 *   - HBA init/reset/probe
 *   - Port state machine + DMA buffers
 *   - ATA commands (IDENTIFY, DMA R/W, FLUSH, SET FEATURES)
 *   - Blockdriver table + SEF lifecycle callbacks (SEF init, signal handling)
 *   - Interrupt handler
 *
 * Build: linked via add_rust_library(minix-ahci) CMake function + add_minix_service()
 */

#include <minix/drivers.h>

/* Rust extern "C" entry point — defined in rust/minix-ahci/src/lib.rs
 *
 * Replaces the original C main() by performing the same sequence:
 *   env_setargs() → sef_setcb_init_fresh() → sef_setcb_signal_handler()
 *   → sef_startup() → blockdriver_mt_task()
 *
 * Returns 0 on success, negative errno on failure.
 */
extern int ahci_rust_main(int argc, char **argv);

int main(int argc, char **argv)
{
	return ahci_rust_main(argc, argv);
}

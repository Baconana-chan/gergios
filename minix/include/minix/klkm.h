/* klkm.h — GergiOS Kernel Loadable Kernel Module Manager IPC Protocol
 *
 * Defines the IPC message types and field accessors for communication
 * between the KLKM daemon and userspace tools (modprobe, lsmod, etc.).
 *
 * Message types use the 0x1B00-0x1BFF range (allocated above NDEV).
 *
 * Protocol:
 *   Tool sends request → KLKM processes → Tool receives reply
 *   m_type carries the KLKM command
 *   Reply m_type = 0 on success, negative errno on error
 *
 * Fields (via m3 struct: 44-byte ca1 for strings, m1_i1/m1_i2 for ints):
 *
 *   KLKM_LOAD_NAME  — load driver by name (looks up in modprobe config)
 *     req: m_type=KLKM_LOAD_NAME, m3_ca1=driver_name
 *
 *   KLKM_LOAD_KO    — load .ko file by path
 *     req: m_type=KLKM_LOAD_KO, m3_ca1=ko_path
 *
 *   KLKM_UNLOAD     — unload module by name
 *     req: m_type=KLKM_UNLOAD, m3_ca1=module_name
 *
 *   KLKM_LIST       — list all loaded modules
 *     req: m_type=KLKM_LIST, m1_p1=caller_buffer_vaddr, m1_i2=buffer_size
 *     resp: daemon writes formatted list to caller buffer via sys_datacopy,
 *           then sets m1_i1=module_count. m_type=0 on success.
 *
 *   KLKM_STATUS     — get status of a specific module
 *     req: m_type=KLKM_STATUS, m3_ca1=module_name
 *     resp: m1_i1=module_state, m3_ca1=status_string
 *
 *   KLKM_COUNT      — get count of loaded modules
 *     req: m_type=KLKM_COUNT
 *     resp: m1_i1=count
 */

#ifndef _MINIX_KLKM_H
#define _MINIX_KLKM_H

/*===========================================================================*
 *      Message types (0x1B00-0x1BFF reserved for KLKM)                    *
 *===========================================================================*/

#define KLKM_BASE           0x1B00

#define KLKM_LOAD_NAME      (KLKM_BASE + 0)   /* load driver by name */
#define KLKM_LOAD_KO        (KLKM_BASE + 1)   /* load .ko file by path */
#define KLKM_UNLOAD         (KLKM_BASE + 2)   /* unload module by name */
#define KLKM_LIST           (KLKM_BASE + 3)   /* list all loaded modules */
#define KLKM_STATUS         (KLKM_BASE + 4)   /* get module status */
#define KLKM_COUNT          (KLKM_BASE + 5)   /* get module count */

#define KLKM_CMD_MASK       0xFF
#define KLKM_IS_CMD(t)      (((t) & ~(KLKM_CMD_MASK)) == KLKM_BASE)

/*===========================================================================*
 *      Field names (using m1 for integers, m3 for char array)              *
 *===========================================================================*/

/* For all requests */
#define KLKM_CMD            m_type
#define KLKM_CALLER         m_source

/* For LOAD_NAME / LOAD_KO / UNLOAD / STATUS:
 *   module name/path passed in m3_ca1 (44 bytes) */
#define KLKM_STR            m3_ca1
#define KLKM_STR_MAX        44

/* For LIST:
 *   caller provides buffer vaddr + size (daemon writes via sys_datacopy) */
#define KLKM_BUF            m1_p1           /* caller buffer virtual addr */
#define KLKM_BUF_SIZE       m1_i2           /* buffer size in bytes */

/* Reply fields */
#define KLKM_RESULT         m_type          /* 0 = OK, negative = errno */
#define KLKM_COUNT_VAL      m1_i1           /* module count */
#define KLKM_STATE_VAL      m1_i1           /* module state from list */
#define KLKM_RESP_STR       m3_ca1          /* response string (44 bytes) */

/*===========================================================================*
 *      Service name for DS registration                                    *
 *===========================================================================*/

#define KLKM_SERVICE_NAME   "klkm"

#endif /* _MINIX_KLKM_H */

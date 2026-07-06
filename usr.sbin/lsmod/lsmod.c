/* lsmod — list loaded kernel modules
 *
 * Sends a KLKM_LIST request to the KLKM daemon with a local buffer
 * for the formatted module list output.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <minix/ipc.h>
#include <minix/endpoint.h>
#include <minix/rs.h>
#include <minix/klkm.h>

int main(int argc __unused, char *argv[] __unused)
{
    endpoint_t klkm_ep;
    message m;
    char list_buf[4096];
    int r;

    /* Find the KLKM daemon */
    if (minix_rs_lookup(KLKM_SERVICE_NAME, &klkm_ep) != OK) {
        fprintf(stderr, "lsmod: KLKM service not found. "
            "Is 'klkm' running?\n");
        return 1;
    }

    /* Send LIST request with local buffer */
    memset(&m, 0, sizeof(m));
    m.m_type = KLKM_LIST;
    m.KLKM_BUF = (char *)list_buf;
    m.KLKM_BUF_SIZE = sizeof(list_buf);

    r = ipc_sendrec(klkm_ep, &m);
    if (r != OK) {
        fprintf(stderr, "lsmod: IPC error: %d\n", r);
        return 1;
    }

    if (m.m_type != 0) {
        fprintf(stderr, "lsmod: request failed: %d (%s)\n",
            m.m_type, m.m_type < 0 ? strerror(-m.m_type) : "unknown");
        return 1;
    }

    /* Print the response */
    printf("Module                  State       Refcount  Type\n");
    printf("---------------------- ----------  --------- -----\n");
    printf("%s", list_buf);

    if (m.KLKM_COUNT_VAL == 0)
        printf("(no modules loaded)\n");

    printf("\nTotal: %d module(s)\n", m.KLKM_COUNT_VAL);

    return 0;
}

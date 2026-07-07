#ifndef MINIX_NET_LWIP_DTLS_SYSCTL_H
#define MINIX_NET_LWIP_DTLS_SYSCTL_H

/*
 * Initialize the DTLS sysctl tree (minix.lwip.dtls).
 * Must be called before mibtree_init().
 */
void dtls_sysctl_init(void);

#endif /* !MINIX_NET_LWIP_DTLS_SYSCTL_H */

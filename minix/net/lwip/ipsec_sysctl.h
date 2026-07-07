#ifndef MINIX_NET_LWIP_IPSEC_SYSCTL_H
#define MINIX_NET_LWIP_IPSEC_SYSCTL_H

/*
 * Initialize the IPsec sysctl tree (minix.lwip.ipsec).
 * Must be called before mibtree_init().
 */
void ipsec_sysctl_init(void);

#endif /* !MINIX_NET_LWIP_IPSEC_SYSCTL_H */

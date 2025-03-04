/**
 * Simple server test program.
 *
 * Listens on the given port, and accepts one connection on it.
 *
 */

#include <assert.h>
#include <netdb.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <unistd.h>

#include <openssl/ssl.h>
#include <openssl/x509.h>

#include "helpers.h"

int main(int argc, char **argv) {
  if (argc != 5) {
    printf("%s <port> <key-file> <cert-chain-file> <cacert>|unauth\n\n",
           argv[0]);
    return 1;
  }

  const char *port = argv[1], *keyfile = argv[2], *certfile = argv[3],
             *cacert = argv[4];

  int listener = TRACE(socket(AF_INET, SOCK_STREAM, 0));
  struct sockaddr_in us, them;
  memset(&us, 0, sizeof(us));
  us.sin_family = AF_INET;
  us.sin_addr.s_addr = htonl(INADDR_ANY);
  us.sin_port = htons(atoi(port));
  TRACE(bind(listener, (struct sockaddr *)&us, sizeof(us)));
  TRACE(listen(listener, 5));
  printf("listening\n");
  fflush(stdout);
  socklen_t them_len = sizeof(them);
  int sock = TRACE(accept(listener, (struct sockaddr *)&them, &them_len));

  TRACE(OPENSSL_init_ssl(0, NULL));
  dump_openssl_error_stack();
  SSL_CTX *ctx = SSL_CTX_new(TLS_method());
  dump_openssl_error_stack();
  if (strcmp(cacert, "unauth") != 0) {
    SSL_CTX_set_verify(ctx, SSL_VERIFY_PEER, NULL);
    dump_openssl_error_stack();
    TRACE(SSL_CTX_load_verify_file(ctx, cacert));
    dump_openssl_error_stack();
  } else {
    printf("client auth disabled\n");
  }

  X509 *server_cert = NULL;
  EVP_PKEY *server_key = NULL;
  TRACE(SSL_CTX_use_certificate_chain_file(ctx, certfile));
  TRACE(SSL_CTX_use_PrivateKey_file(ctx, keyfile, SSL_FILETYPE_PEM));
  server_key = SSL_CTX_get0_privatekey(ctx);
  server_cert = SSL_CTX_get0_certificate(ctx);

  TRACE(SSL_CTX_set_alpn_protos(ctx, (const uint8_t *)"\x08http/1.1", 9));
  dump_openssl_error_stack();

  SSL *ssl = SSL_new(ctx);
  dump_openssl_error_stack();
  printf("SSL_new: SSL_get_privatekey %s SSL_CTX_get0_privatekey\n",
         SSL_get_privatekey(ssl) == server_key ? "same as" : "differs to");
  printf("SSL_new: SSL_get_certificate %s SSL_CTX_get0_certificate\n",
         SSL_get_certificate(ssl) == server_cert ? "same as" : "differs to");
  state(ssl);
  TRACE(SSL_set_fd(ssl, sock));
  dump_openssl_error_stack();
  state(ssl);
  TRACE(SSL_accept(ssl));
  dump_openssl_error_stack();
  state(ssl);
  printf("SSL_accept: SSL_get_privatekey %s SSL_CTX_get0_privatekey\n",
         SSL_get_privatekey(ssl) == server_key ? "same as" : "differs to");
  printf("SSL_accept: SSL_get_certificate %s SSL_CTX_get0_certificate\n",
         SSL_get_certificate(ssl) == server_cert ? "same as" : "differs to");

  // check the alpn (also sees that SSL_accept completed handshake)
  const uint8_t *alpn_ptr = NULL;
  unsigned int alpn_len = 0;
  SSL_get0_alpn_selected(ssl, &alpn_ptr, &alpn_len);
  hexdump("alpn", alpn_ptr, alpn_len);

  printf("version: %s\n", SSL_get_version(ssl));
  printf("verify-result: %ld\n", SSL_get_verify_result(ssl));
  printf("cipher: %s\n", SSL_CIPHER_standard_name(SSL_get_current_cipher(ssl)));

  show_peer_certificate("client", ssl);

  // read "request"
  while (1) {
    char buf[128] = {0};
    int rd = TRACE(SSL_read(ssl, buf, sizeof(buf)));
    dump_openssl_error_stack();
    TRACE(SSL_pending(ssl));
    dump_openssl_error_stack();
    TRACE(SSL_has_pending(ssl));
    dump_openssl_error_stack();
    if (rd == 0) {
      printf("nothing read\n");
      break;
    } else {
      hexdump("result", buf, rd);
    }
    state(ssl);

    if (!TRACE(SSL_has_pending(ssl))) {
      break;
    }
  }

  // write some data and close
  const char response[] = "HTTP/1.0 200 OK\r\n\r\nhello\r\n";
  int wr = TRACE(SSL_write(ssl, response, sizeof(response) - 1));
  dump_openssl_error_stack();
  assert(wr == sizeof(response) - 1);
  TRACE(SSL_shutdown(ssl));
  dump_openssl_error_stack();

  close(sock);
  close(listener);
  SSL_free(ssl);
  SSL_CTX_free(ctx);

  printf("PASS\n\n");
  return 0;
}

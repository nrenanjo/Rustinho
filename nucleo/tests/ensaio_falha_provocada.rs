//! ENSAIO DO PORTÃO (dev.3.3, passo 1.2). Este arquivo só existe na branch
//! descartável `ensaio/ci-falha-provocada` e NUNCA entra na branch de
//! desenvolvimento: o teste abaixo falha de propósito, para provar que, com um
//! job vermelho, o portão não roda e nenhum rascunho de versão é possível.

#[test]
fn falha_provocada_para_o_ensaio_do_portao() {
    panic!("falha PROVOCADA (ensaio do portão, dev.3.3 passo 1.2): o portão não pode rodar");
}

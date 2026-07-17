// Camada de serviço: a regra de negócio da carteira vive aqui, entre os
// handlers HTTP (que só traduzem requisição/resposta) e o repository (que só
// fala SQL). É o ponto natural para regras futuras — taxas, limites por perfil,
// notificações — sem inchar nem os handlers nem as queries.
pub mod portfolio;

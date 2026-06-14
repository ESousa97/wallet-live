use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::CookieJar;
use jwt_simple::prelude::*;
use password_auth::VerifyError;

use crate::app::AppState;
use crate::error::AppError;
use crate::repository::Repository;

/// Chave secreta usada para ASSINAR e VALIDAR os JWTs. Só o back-end a conhece:
/// é o que garante que um token não foi fabricado nem adulterado. Em produção
/// viria de uma variável de ambiente ou cofre de segredos, como a do admin.
const JWT_SECRET_ENV: &str = "JWT_SECRET";

/// Nome do cookie onde o token de sessão é guardado no navegador.
pub const TOKEN_COOKIE: &str = "token";

/// Dados que embutimos no JWT (as "claims" customizadas). Vira JSON, então
/// precisa de Serialize/Deserialize. NÃO é criptografado — qualquer um lê o
/// conteúdo; a assinatura é que prova a autenticidade.
#[derive(Serialize, Deserialize)]
struct UserClaims {
    id: i64,
    username: String,
}

impl From<User> for UserClaims {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
        }
    }
}

/// Um usuário já autenticado no sistema. Diferente do `Admin`, ele carrega dados
/// (id e nome) que usaremos depois para acessar as relações do usuário. Os campos
/// são privados de propósito: a única forma de obter um `User` é passando por um
/// dos fluxos de autenticação abaixo, então um `User` em mãos é prova de que o
/// fluxo foi cumprido. Não guardamos a senha nem a hash: já foi autenticado.
pub struct User {
    id: i64,
    username: String,
}

impl User {
    fn new(id: i64, username: String) -> Self {
        Self { id, username }
    }

    pub const fn id(&self) -> i64 {
        self.id
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    /// Gera o JWT de sessão deste usuário. Consome `self` (move os dados para
    /// dentro das claims). O token é assinado com a `SECRET_KEY` e vale 10 min.
    pub fn auth_token(self) -> Result<String, AppError> {
        let secret = std::env::var(JWT_SECRET_ENV).map_err(|_| AppError::InvalidCredentials)?;
        let key = HS256Key::from_bytes(secret.as_bytes());
        let claims = Claims::with_custom_claims(UserClaims::from(self), Duration::from_mins(10));
        let token = key.authenticate(claims)?;
        Ok(token)
    }

    /// Reconstrói um `User` a partir de um token. Usa a mesma chave para VALIDAR
    /// a assinatura: se o token foi fabricado/adulterado ou expirou, falha. Só
    /// depois dessa verificação confiamos no conteúdo das claims.
    fn from_auth_token(token: &str) -> Result<Self, AppError> {
        let secret = std::env::var(JWT_SECRET_ENV).map_err(|_| AppError::InvalidCredentials)?;
        let key = HS256Key::from_bytes(secret.as_bytes());
        let claims = key.verify_token::<UserClaims>(token, None)?;
        Ok(Self::new(claims.custom.id, claims.custom.username))
    }
}

/// Extrai um `User` autenticado da requisição: lê o cookie `token` (que o
/// navegador reenvia automaticamente) e o valida. Basta anotar um handler com um
/// parâmetro `User` para exigir sessão válida.
impl FromRequestParts<AppState> for User {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // A cookie jar é só uma leitura dos headers da requisição.
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(TOKEN_COOKIE)
            .ok_or(AppError::MissingAuthorization)?
            .value();

        Self::from_auth_token(token)
    }
}

/// Versão tolerante: nunca falha. Útil em telas que devem redirecionar para o
/// login quando não há sessão válida, em vez de devolver um erro. Token ausente,
/// inválido ou expirado viram `None`.
impl FromRequestParts<AppState> for Option<User> {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(User::from_request_parts(parts, state).await.ok())
    }
}

/// Um usuário que ainda NÃO foi autenticado: tem só o nome e a senha em texto
/// livre que vieram do formulário. Pode virar um `User` de duas formas: provando
/// a senha de um usuário existente (`authenticate`) ou se cadastrando como um
/// usuário novo (`register`).
pub struct UnauthenticatedUser {
    username: String,
    password: String,
}

impl UnauthenticatedUser {
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
    }

    /// Tenta autenticar este usuário contra o banco. Encapsulamos toda a lógica
    /// aqui (puxar o registro, conferir a hash) para que os endpoints não
    /// precisem conhecer esses detalhes — eles só repassam o `Repository`.
    pub async fn authenticate(&self, repository: &Repository) -> Result<User, AppError> {
        // Se o usuário não existe, sinalizamos com um erro próprio: o endpoint o
        // trata para decidir entre login e cadastro.
        let user_record = match repository.get_user_by_name(&self.username).await? {
            Some(user_record) => user_record,
            None => return Err(AppError::UserDoesNotExist),
        };

        // Não sabemos (by design) como a hash é verificada — delegamos para a
        // biblioteca, que escolhe o algoritmo adequado.
        match password_auth::verify_password(&self.password, &user_record.password_hash) {
            Ok(()) => Ok(User::new(user_record.id, user_record.username)),
            Err(VerifyError::PasswordInvalid) => Err(AppError::InvalidCredentials),
            // A hash guardada inclui o algoritmo usado; se ela não parseia, algo
            // muito errado aconteceu (usuário inserido por outra via). Melhor
            // derrubar o programa do que continuar num estado inconsistente.
            Err(VerifyError::Parse(error)) => {
                panic!("failed to parse stored password hash: {error}");
            }
        }
    }

    /// Cadastra este usuário como novo. Consome `self` para mover os dados direto
    /// para o banco. Gera a hash da senha aqui — o repository nunca vê texto livre.
    pub async fn register(self, repository: &Repository) -> Result<User, AppError> {
        let password_hash = password_auth::generate_hash(&self.password);

        match repository.add_user(self.username, password_hash).await {
            Ok(user_record) => Ok(User::new(user_record.id, user_record.username)),
            // O username é UNIQUE: violar a constraint vira um erro específico e
            // mais útil do que um 500 genérico de banco.
            Err(sqlx::Error::Database(db_error)) if db_error.is_unique_violation() => {
                Err(AppError::UsernameTaken)
            }
            Err(error) => Err(AppError::Database(error)),
        }
    }
}

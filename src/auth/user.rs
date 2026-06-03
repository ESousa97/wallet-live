use password_auth::VerifyError;

use crate::error::AppError;
use crate::repository::Repository;

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

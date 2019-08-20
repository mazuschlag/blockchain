use std::io;
use std::sync::Mutex;
use chrono::prelude::*;
use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};
use actix_web::{middleware, web, post, get, App, HttpResponse, HttpServer};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Response {
    message: String
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct FullChain {
    chain: Vec<Block>,
    length: usize
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Transaction {
    sender: String,
    recipient: String,
    amount: f32
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Mine {
    message: String,
    index: usize,
    transactions: Vec<Transaction>,
    proof: usize,
    previous_hash: String
}

impl Transaction {
    fn new(sender: &str, recipient: &str, amount: f32) -> Transaction {
        Transaction {
            sender: sender.to_string(),
            recipient: recipient.to_string(),
            amount: amount
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Block {
    index: usize,
    timestamp: String,
    transactions: Vec<Transaction>,
    proof: usize,
    previous_hash: String
}

impl Block {
    fn new(index: usize, transactions: Vec<Transaction>, proof: usize, previous_hash: &str) -> Block {
        Block {
            index: index,
            timestamp: format!("{}", Utc::now()),
            transactions: transactions,
            proof: proof,
            previous_hash: previous_hash.to_string()
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Blockchain {
    current_transactions: Vec<Transaction>,
    chain: Vec<Block>
}

impl Blockchain {
    fn new() -> Blockchain {
        let mut blockchain = Blockchain{ current_transactions: Vec::new(), chain: Vec::new() };
        let prev_hash = format!("{:x}", Sha256::new().chain(b"1").result());
        blockchain.new_block(1, &prev_hash);
        blockchain
    }

    fn new_block(&mut self, proof: usize, prev_hash: &str) -> &Block {
        let block = Block::new(self.chain.len() + 1, self.current_transactions.clone(), proof, prev_hash);
        self.current_transactions = Vec::new();
        self.chain.push(block);
        &self.chain[self.chain.len()-1]
    }

    fn new_transaction(&mut self, sender: &str, recipient: &str, amount: f32) -> usize {
        let transaction = Transaction::new(sender, recipient, amount);
        self.current_transactions.push(transaction);
        match self.chain.last_mut() {
            Some(block) => block.index + 1,
            None => 0
        }
    }

    fn hash(block: &Block) -> String {
        let block_string = serde_json::to_string(block).unwrap();
        format!("{:x}", Sha256::new().chain(block_string).result())
    }

    fn proof_of_work(&self, last_block: &Block) -> usize {
        let last_proof = last_block.proof;
        let last_hash = Blockchain::hash(last_block);
        let mut proof = 0;
        while !Blockchain::valid_proof(last_proof, proof, last_hash.as_str()) {
            proof += 1;
        }
        proof
    }

    fn valid_proof(last_proof: usize, proof: usize, last_hash: &str) -> bool {
        let guess = format!("{}{}{}", last_proof, proof, last_hash);
        let guess_hash = format!("{:x}", Sha256::new().chain(guess).result());
        &guess_hash[..5] == "00000"
    }

    fn full_chain(&self) -> FullChain {
        FullChain {
            chain: self.chain.clone(),
            length: self.chain.len()
        }
    }
}

#[get("/mine")]
fn mine(blockchain: web::Data<Mutex<Blockchain>>) -> HttpResponse {
    let mut local_blockchain = blockchain.lock().unwrap();
    if let Some(last_block) = local_blockchain.chain.last() {
        let proof = local_blockchain.proof_of_work(last_block);
        let previous_hash = Blockchain::hash(last_block);
        let block = local_blockchain.new_block(proof, &previous_hash);
        return HttpResponse::Ok().json(Mine {
            message: "New block forged".to_string(),
            index: block.index,
            transactions: block.transactions.clone(),
            proof: proof,
            previous_hash: previous_hash
        })
    }
    HttpResponse::Ok().json(Response {
        message: "There was an error mining".to_string()
    })
}

#[post("/transactions/new")]
fn new_transaction(blockchain: web::Data<Mutex<Blockchain>>, req: web::Json<Transaction>) -> HttpResponse {
    let index = blockchain.lock().unwrap().new_transaction(&req.sender, &req.recipient, req.amount);
    HttpResponse::Ok().json(Response {
        message: format!("Your transaction will be in block: {}", index)
    })
}

#[get("/chain")]
fn full_chain(blockchain: web::Data<Mutex<Blockchain>>) -> HttpResponse {
    HttpResponse::Ok().json(blockchain.lock().unwrap().full_chain())
}


fn main() -> io::Result<()> {
    let blockchain = web::Data::new(Mutex::new(Blockchain::new()));
    HttpServer::new(move || {
        App::new()
            .register_data(blockchain.clone())
            .wrap(middleware::Logger::default())
            .service(new_transaction)
            .service(full_chain)
            .service(mine)
    })
    .bind("127.0.0.1:3000")?
    .run()
}

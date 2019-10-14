use std::env;
use std::io;
use std::sync::Mutex;
use std::collections:: HashSet;
use actix_web::{middleware, web, post, get, App, HttpResponse, HttpServer};
use chrono::prelude::*;
use reqwest;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use url::{Url};
use uuid::Uuid;

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
struct NodeList {
    nodes: Vec<String>,
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

    fn hash(&self) -> String {
        let block_string = serde_json::to_string(self).unwrap();
        format!("{:x}", Sha256::new().chain(block_string).result())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Blockchain {
    current_transactions: Vec<Transaction>,
    chain: Vec<Block>,
    nodes: HashSet<String>
}

impl Blockchain {
    fn new() -> Blockchain {
        let mut blockchain = Blockchain{ current_transactions: Vec::new(), chain: Vec::new(), nodes: HashSet::new() };
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

    fn register_node(&mut self, node: &str) -> bool {
        let parsed_url = Url::parse(node).unwrap();
        if let Some(host) = parsed_url.host_str() {
            if let Some(port) = parsed_url.port() {
                return self.nodes.insert(format!("{}:{}", host, port))
            } else {
                return self.nodes.insert(format!("{}", host))
            }
        }
        false
    }

    fn proof_of_work(&self, last_block: &Block) -> usize {
        let last_proof = last_block.proof;
        let last_hash = last_block.hash();
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

    fn valid_chain(chain: &Vec<Block>) -> bool {
        match chain.first() {
            Some(mut prev_block) => {
                let prev_block_hash = prev_block.hash();
                for block in chain.iter().skip(1) {
                    println!("previous block: {:?}", prev_block);
                    println!("current block: {:?}", block);
                    println!("----------------");
                    if block.previous_hash != prev_block_hash {
                        return false
                    }
                    if !Blockchain::valid_proof(prev_block.proof, block.proof, &prev_block_hash) {
                        return false
                    }
                    prev_block = block;
                }
                return true
            },
            None => return false
        }
    }

    fn resolve_conflicts(&mut self) {
        for node in &self.nodes {
            let res: FullChain = reqwest::get(&format!("http://{}/chain", node)).unwrap().json().unwrap();
            if res.length > self.chain.len() && Blockchain::valid_chain(&res.chain) {
                self.chain = res.chain;
            }
        }
    }

    fn full_chain(&self) -> FullChain {
        FullChain {
            chain: self.chain.clone(),
            length: self.chain.len()
        }
    }

    fn node_list(&self) -> NodeList {
        let mut node_list = Vec::new();
        for node in self.nodes.iter() {
            node_list.push(node.to_string());
        }
        NodeList {
            nodes: node_list
        }
    }
}

#[get("/mine")]
fn mine(blockchain: web::Data<Mutex<Blockchain>>) -> HttpResponse {
    let mut local_blockchain = blockchain.lock().unwrap();
    if let Some(last_block) = local_blockchain.chain.last() {
        let proof = local_blockchain.proof_of_work(last_block);
        let previous_hash = last_block.hash();
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

#[post("/nodes/register")]
fn register_nodes(blockchain: web::Data<Mutex<Blockchain>>, req: web::Json<NodeList>) -> HttpResponse {
    for node in &req.nodes {
        let _ = blockchain.lock().unwrap().register_node(node);
    }
    HttpResponse::Ok().json(Response {
        message: format!("Nodes successfully registered")
    })
}

#[get("/nodes/resolve")]
fn consensus(blockchain: web::Data<Mutex<Blockchain>>) -> HttpResponse {
    blockchain.lock().unwrap().resolve_conflicts();
    HttpResponse::Ok().json(Response {
        message: format!("Conflicts resolved")
    })
}

#[get("/nodes")]
fn nodes(blockchain: web::Data<Mutex<Blockchain>>) -> HttpResponse {
    HttpResponse::Ok().json(blockchain.lock().unwrap().node_list())
}


fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let port = &args[1];
    let _ = format!("{}", Uuid::new_v4()).replace("-", "");
    let blockchain = web::Data::new(Mutex::new(Blockchain::new()));
    HttpServer::new(move || {
        App::new()
            .register_data(blockchain.clone())
            .wrap(middleware::Logger::default())
            .service(new_transaction)
            .service(full_chain)
            .service(mine)
            .service(nodes)
            .service(register_nodes)
            .service(consensus)
    })
    .bind(format!("127.0.0.1:{}", port))?
    .run()
}

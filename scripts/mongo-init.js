// MongoDB initialization script for Virtual DOM Gateway

// Switch to virtual_dom database
db = db.getSiblingDB('virtual_dom');

// Create collections with validation
db.createCollection('documents', {
  validator: {
    $jsonSchema: {
      bsonType: 'object',
      required: ['repo', 'branch', 'path', 'blob', 'author', '_v', 'timestamp', 'type'],
      properties: {
        repo: {
          bsonType: 'string',
          description: 'Repository name'
        },
        branch: {
          bsonType: 'string',
          description: 'Branch name'
        },
        path: {
          bsonType: 'string',
          description: 'File path'
        },
        blob: {
          bsonType: 'binData',
          description: 'Binary content'
        },
        author: {
          bsonType: 'string',
          description: 'Author identifier'
        },
        _v: {
          bsonType: 'object',
          required: ['value'],
          properties: {
            value: {
              bsonType: 'long',
              description: 'Vector clock value'
            }
          }
        },
        timestamp: {
          bsonType: 'date',
          description: 'Timestamp'
        },
        type: {
          bsonType: 'string',
          description: 'Document type'
        },
        metadata: {
          bsonType: 'object',
          description: 'Additional metadata'
        }
      }
    }
  }
});

db.createCollection('push_intents', {
  validator: {
    $jsonSchema: {
      bsonType: 'object',
      required: ['repo', 'branch', 'author', 'message', 'timestamp', 'processed', 'documents'],
      properties: {
        repo: {
          bsonType: 'string',
          description: 'Repository name'
        },
        branch: {
          bsonType: 'string',
          description: 'Branch name'
        },
        author: {
          bsonType: 'string',
          description: 'Author identifier'
        },
        message: {
          bsonType: 'string',
          description: 'Commit message'
        },
        timestamp: {
          bsonType: 'date',
          description: 'Created timestamp'
        },
        processed: {
          bsonType: 'bool',
          description: 'Processing status'
        },
        processed_at: {
          bsonType: 'date',
          description: 'Processing timestamp'
        },
        error: {
          bsonType: 'string',
          description: 'Error message if failed'
        },
        documents: {
          bsonType: 'array',
          description: 'Document IDs to push',
          items: {
            bsonType: 'string'
          }
        }
      }
    }
  }
});

db.createCollection('history');
db.createCollection('conflicts');

// Create indexes
db.documents.createIndex({ repo: 1, branch: 1, path: 1 }, { unique: true });
db.documents.createIndex({ timestamp: -1 });
db.documents.createIndex({ author: 1 });
db.documents.createIndex({ '_v.value': 1 });

db.push_intents.createIndex({ processed: 1, timestamp: 1 });
db.push_intents.createIndex({ repo: 1, branch: 1 });
db.push_intents.createIndex({ timestamp: -1 });
db.push_intents.createIndex({ author: 1 });

db.history.createIndex({ repo: 1, branch: 1, path: 1, version: -1 });
db.history.createIndex({ timestamp: -1 });

db.conflicts.createIndex({ repo: 1, branch: 1, path: 1 });
db.conflicts.createIndex({ resolved: 1 });
db.conflicts.createIndex({ created_at: -1 });

print('Virtual DOM database initialized successfully');
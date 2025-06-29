package mongodb

import (
	"context"
	"fmt"
	"time"

	"go.mongodb.org/mongo-driver/bson"
	"go.mongodb.org/mongo-driver/mongo"
	"go.mongodb.org/mongo-driver/mongo/options"
	"go.mongodb.org/mongo-driver/mongo/readpref"
)

// Document represents a document in the virtual DOM
type Document struct {
	ID        string                 `bson:"_id,omitempty"`
	Repo      string                 `bson:"repo"`
	Branch    string                 `bson:"branch"`
	Path      string                 `bson:"path"`
	Blob      []byte                 `bson:"blob"`
	Author    string                 `bson:"author"`
	Version   int64                  `bson:"_v"`
	Timestamp time.Time              `bson:"timestamp"`
	Type      string                 `bson:"type"`
	Metadata  map[string]interface{} `bson:"metadata"`
}

// PushIntent represents a push intent document
type PushIntent struct {
	ID         string    `bson:"_id,omitempty"`
	Repo       string    `bson:"repo"`
	Branch     string    `bson:"branch"`
	Author     string    `bson:"author"`
	Message    string    `bson:"message"`
	Timestamp  time.Time `bson:"timestamp"`
	Processed  bool      `bson:"processed"`
	ProcessedAt *time.Time `bson:"processed_at,omitempty"`
	Error      string    `bson:"error,omitempty"`
	Documents  []string  `bson:"documents"` // Document IDs
}

// Client wraps MongoDB operations
type Client struct {
	client   *mongo.Client
	database *mongo.Database
}

// NewClient creates a new MongoDB client
func NewClient(ctx context.Context, uri, databaseName string) (*Client, error) {
	clientOptions := options.Client().
		ApplyURI(uri).
		SetServerAPIOptions(options.ServerAPI(options.ServerAPIVersion1))

	client, err := mongo.Connect(ctx, clientOptions)
	if err != nil {
		return nil, fmt.Errorf("failed to connect to MongoDB: %w", err)
	}

	// Ping to verify connection
	ctx, cancel := context.WithTimeout(ctx, 5*time.Second)
	defer cancel()
	
	if err := client.Ping(ctx, readpref.Primary()); err != nil {
		return nil, fmt.Errorf("failed to ping MongoDB: %w", err)
	}

	return &Client{
		client:   client,
		database: client.Database(databaseName),
	}, nil
}

// Close closes the MongoDB connection
func (c *Client) Close(ctx context.Context) error {
	return c.client.Disconnect(ctx)
}

// GetPendingPushIntents retrieves unprocessed push intents
func (c *Client) GetPendingPushIntents(ctx context.Context, limit int) ([]*PushIntent, error) {
	collection := c.database.Collection("push_intents")
	
	filter := bson.M{"processed": false}
	opts := options.Find().
		SetSort(bson.D{{Key: "timestamp", Value: 1}}).
		SetLimit(int64(limit))

	cursor, err := collection.Find(ctx, filter, opts)
	if err != nil {
		return nil, fmt.Errorf("failed to find push intents: %w", err)
	}
	defer cursor.Close(ctx)

	var intents []*PushIntent
	if err := cursor.All(ctx, &intents); err != nil {
		return nil, fmt.Errorf("failed to decode push intents: %w", err)
	}

	return intents, nil
}

// GetDocumentsByIDs retrieves documents by their IDs
func (c *Client) GetDocumentsByIDs(ctx context.Context, ids []string) ([]*Document, error) {
	collection := c.database.Collection("documents")
	
	filter := bson.M{"_id": bson.M{"$in": ids}}
	
	cursor, err := collection.Find(ctx, filter)
	if err != nil {
		return nil, fmt.Errorf("failed to find documents: %w", err)
	}
	defer cursor.Close(ctx)

	var documents []*Document
	if err := cursor.All(ctx, &documents); err != nil {
		return nil, fmt.Errorf("failed to decode documents: %w", err)
	}

	return documents, nil
}

// MarkPushIntentProcessed marks a push intent as processed
func (c *Client) MarkPushIntentProcessed(ctx context.Context, id string, err error) error {
	collection := c.database.Collection("push_intents")
	
	now := time.Now()
	update := bson.M{
		"$set": bson.M{
			"processed":    true,
			"processed_at": now,
		},
	}

	if err != nil {
		update["$set"].(bson.M)["error"] = err.Error()
	}

	result, updateErr := collection.UpdateOne(
		ctx,
		bson.M{"_id": id},
		update,
	)

	if updateErr != nil {
		return fmt.Errorf("failed to update push intent: %w", updateErr)
	}

	if result.MatchedCount == 0 {
		return fmt.Errorf("push intent not found: %s", id)
	}

	return nil
}

// WatchPushIntents creates a change stream for push intents
func (c *Client) WatchPushIntents(ctx context.Context) (*mongo.ChangeStream, error) {
	collection := c.database.Collection("push_intents")
	
	pipeline := mongo.Pipeline{
		{{Key: "$match", Value: bson.D{
			{Key: "operationType", Value: "insert"},
			{Key: "fullDocument.processed", Value: false},
		}}},
	}

	opts := options.ChangeStream().
		SetFullDocument(options.UpdateLookup)

	stream, err := collection.Watch(ctx, pipeline, opts)
	if err != nil {
		return nil, fmt.Errorf("failed to create change stream: %w", err)
	}

	return stream, nil
}

// CreateIndexes creates necessary indexes
func (c *Client) CreateIndexes(ctx context.Context) error {
	// Push intents indexes
	pushIntentsCol := c.database.Collection("push_intents")
	pushIntentsIndexes := []mongo.IndexModel{
		{
			Keys: bson.D{
				{Key: "processed", Value: 1},
				{Key: "timestamp", Value: 1},
			},
		},
		{
			Keys: bson.D{{Key: "repo", Value: 1}},
		},
		{
			Keys: bson.D{{Key: "branch", Value: 1}},
		},
	}

	if _, err := pushIntentsCol.Indexes().CreateMany(ctx, pushIntentsIndexes); err != nil {
		return fmt.Errorf("failed to create push_intents indexes: %w", err)
	}

	// Documents indexes (if needed for queries)
	documentsCol := c.database.Collection("documents")
	documentsIndexes := []mongo.IndexModel{
		{
			Keys: bson.D{
				{Key: "repo", Value: 1},
				{Key: "branch", Value: 1},
				{Key: "path", Value: 1},
			},
			Options: options.Index().SetUnique(true),
		},
		{
			Keys: bson.D{{Key: "timestamp", Value: -1}},
		},
	}

	if _, err := documentsCol.Indexes().CreateMany(ctx, documentsIndexes); err != nil {
		return fmt.Errorf("failed to create documents indexes: %w", err)
	}

	return nil
}
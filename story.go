package main

import (
	"time"
)

// Story is a complete story, told by many people.
type Story struct {
	Snippets []Snippet
	Started  *time.Time
	Finished *time.Time
}

// Snippet is a part of a Story told by a single author.
type Snippet struct {
	Author  string
	Created time.Time
	Content *string
}

// NewStory begins an empty Story.
func NewStory() *Story {
	ts := time.Now()

	return &Story{
		Snippets: make([]Snippet, 3),
		Started:  &ts,
		Finished: nil,
	}
}

// NewSnippet creates a new Snippet.
func NewSnippet(author string, content *string) *Snippet {
	return &Snippet{
		Author:  author,
		Created: time.Now(),
		Content: content,
	}
}

// AppendSnippet appends a new Snippet to an existing Story.
func (story *Story) AppendSnippet(snippet Snippet) {
	story.Snippets = append(story.Snippets, snippet)
}

// FinishStory marks a Story as completed.
func (story *Story) FinishStory() {
	ts := time.Now()
	story.Finished = &ts
}

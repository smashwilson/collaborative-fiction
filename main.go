package main

import (
	"html/template"
	"log"
	"net/http"
	"strings"

	"github.com/kelseyhightower/envconfig"
	"github.com/stretchr/goweb"
	"github.com/stretchr/goweb/context"
)

// Configuration contains application settings and secrets acquired from the environment.
type Configuration struct {
	BaseURL      string
	GoogleKey    string
	GoogleSecret string
	GitHubKey    string
	GitHubSecret string
	Root         string
}

var (
	ts = template.Must(template.ParseFiles(
		"templates/login.html",
		"templates/snippet-form.html",
		"templates/welcome.html",
	))
	story  *Story
	config Configuration
)

func useTemplate(ctx context.Context, templateName string, data interface{}) error {
	err := ts.ExecuteTemplate(ctx.HttpResponseWriter(), templateName, data)
	if err != nil {
		log.Printf("Unable to generate template: %v", err)
		return goweb.Respond.With(ctx, http.StatusInternalServerError, []byte(err.Error()))
	}
	return nil
}

func absURL(subpath string) string {
	if config.Root == "" {
		return strings.Join([]string{config.BaseURL, subpath}, "/")
	}
	return strings.Join([]string{config.BaseURL, config.Root, subpath}, "/")
}

func path(subpath string) string {
	return strings.Join([]string{config.Root, subpath}, "/")
}

func loginHandler(ctx context.Context) error {
	type context struct {
		Root string
	}

	c := context{Root: config.Root}
	return useTemplate(ctx, "login.html", c)
}

func welcomeHandler(ctx context.Context) error {
	type context struct {
		Root   string
		Name   string
		Email  string
		Avatar string
	}

	must := func(str string, err error) string {
		if err != nil {
			return err.Error()
		}
		return str
	}

	c := context{
		Root:   config.Root,
		Name:   must(UserName(ctx)),
		Email:  must(UserEmail(ctx)),
		Avatar: must(UserAvatar(ctx)),
	}
	return useTemplate(ctx, "welcome.html", c)
}

func main() {
	err := envconfig.Process("fiction", &config)
	if err != nil {
		log.Fatalf("Error reading configuration: %v", err)
	}

	if config.BaseURL == "" {
		config.BaseURL = "http://localhost:8080"
	}

	config.BaseURL = strings.TrimRight(config.BaseURL, "/")
	config.Root = strings.TrimRight(config.Root, "/")

	// Summarize the currently active configuration settings, without dumping secrets.
	log.Println("Current configuration:")
	log.Printf("  base url: %s\n", config.BaseURL)
	log.Printf("  root: %s\n", config.Root)
	log.Printf("  Google key [%t] secret [%t]\n", config.GoogleKey != "", config.GoogleSecret != "")
	log.Printf("  GitHub key [%t] secret [%t]\n", config.GitHubKey != "", config.GitHubSecret != "")

	err = registerAuthRoutes()
	if err != nil {
		log.Fatalf("Unable to register auth routes: %v", err)
		return
	}

	goweb.Map("GET", path(""), loginHandler)
	goweb.Map("GET", path("welcome"), welcomeHandler)

	log.Println("Ready to serve.")

	http.ListenAndServe(":8080", goweb.DefaultHttpHandler())
}

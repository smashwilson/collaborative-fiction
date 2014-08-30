package main

import (
	"html/template"
	"log"
	"net/http"
	"strings"

	"github.com/kelseyhightower/envconfig"
	"github.com/stretchr/goweb"
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
	))
	story  *Story
	config Configuration
)

func useTemplate(w http.ResponseWriter, templateName string, data interface{}) {
	err := ts.ExecuteTemplate(w, templateName, data)
	if err != nil {
		log.Printf("Unable to generate template: %v", err)
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
}

func absURL(subpath string) string {
	return config.BaseURL + config.Root + subpath
}

func path(subpath string) string {
	return config.Root + subpath
}

func welcomeHandler(w http.ResponseWriter, r *http.Request) {
	useTemplate(w, "login.html", nil)
}

func snippetFormHandler(w http.ResponseWriter, r *http.Request) {
	useTemplate(w, "snippet-form.html", nil)
}

func snippetSubmitHandler(w http.ResponseWriter, r *http.Request) {
	http.Redirect(w, r, "/", 303)
}

func main() {
	err := envconfig.Process("fiction", &config)
	if err != nil {
		log.Fatalf("Error reading configuration: %v", err)
	}

	if config.BaseURL == "" {
		config.BaseURL = "http://localhost:8080/"
	}

	if !strings.HasSuffix(config.BaseURL, "/") {
		config.BaseURL = config.BaseURL + "/"
	}
	if !strings.HasSuffix(config.Root, "/") {
		config.Root = config.Root + "/"
	}

	registerAuthRoutes()

	http.ListenAndServe(":8080", goweb.DefaultHttpHandler())
}

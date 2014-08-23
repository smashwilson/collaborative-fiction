# Collaborative Fiction generator

This is a web application that allows a group of people to create a work of fiction together, one set of paragraphs at a time. Each contributor sees only the submission of the contributor immediately before them, until the story is complete. Once it's marked as "done" (which is an arbitrary call by the admin), everyone can read the entire thing.

The results can range from nonsensical to hilarious, depending on your friends.

## Installation

1. Install [Docker](https://docs.docker.com/installation/#installation) by following the instructions appropriate to your operating system.
2. Build the image:

   ```bash
   sudo docker build -t fiction .
   ```

3. Run the image:

   ```bash
   sudo docker run -d -p 9000:9000 --name fiction fiction
   ```
4. Point your browser at http://localhost:9000/.

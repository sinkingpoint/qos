# bus

Bus is a daemon that allows many to many communications between services. It does so through the concept of a "topic", which can be written to, or read from. Topics are similar to Kafka topics with a few notable exceptions: 
  
- No historical topic data is stored - Bus assumes that messages in a topic are only immediately interesting, and so only plays messages against currently listening readers. If there are no readers reading from a topic when a message is written, that messages is dropped.
- 
global using System;
using Collections = System.Collections.Generic;

namespace Valknut.Fixtures;

public interface IGreeter
{
    string Greet(string name);
}

public record Person(string Name);

public sealed class Greeter : IGreeter
{
    public string Greet(string name)
    {
        var person = new Person(name);
        Console.WriteLine(person.Name);
        return person.Name.ToUpperInvariant();
    }
}

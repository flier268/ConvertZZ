using System;
using System.Collections.ObjectModel;
using System.Linq;
using PropertyChanged;

namespace ConvertZZ.ViewModels
{
    [AddINotifyPropertyChangedInterface]
    public class Node : ICloneable
    {
        public Node(Node? parent)
        {
            if (parent == null)
                Generation = 1;
            else
                Generation = parent.Generation + 1;
            this.Parent = parent;
        }

        public string? DisplayName { get; set; }

        public bool IsChecked { get; set; }
        public bool IsFile { get; set; }
        private Node? _Parent;

        public Node? Parent
        {
            get => _Parent;
            private set
            {
                if (value == null)
                    Generation = 1;
                else
                {
                    Generation = value.Generation + 1;
                    if (Nodes != null)
                        foreach (Node child in Nodes)
                            child.Parent = this;
                }
                _Parent = value;
            }
        }

        public int Generation { get; private set; }
        private ObservableCollection<Node> _Nodes = new();

        public ObservableCollection<Node> Nodes
        {
            get => _Nodes;
            set
            {
                _Nodes = value;
                if (value != null)
                    foreach (var child in _Nodes)
                        child.Parent = this;
            }
        }

        public object Clone()
        {
            var temp = MemberwiseClone() as Node;
            if (temp is not null)
            {
                temp.Nodes = new(Nodes.Select(x =>
                {
                    if (x.Clone() is Node node)
                    {
                        node._Parent = temp;
                        return node;
                    }
                    throw new InvalidOperationException();
                }).ToList());
            }
            else
                throw new InvalidOperationException();
            return temp;
        }
    }
}
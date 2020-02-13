using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Windows.Controls;
using System.Windows.Input;

namespace ConvertZZ
{
    /// <summary>
    /// CheckboxTreeView.xaml 的互動邏輯
    /// </summary>
    public partial class CheckboxTreeView : UserControl, INotifyPropertyChanged
    {
        public CheckboxTreeView()
        {
            this.DataContext = this;
            InitializeComponent();
        }
        public event PropertyChangedEventHandler PropertyChanged;


        public delegate void CheckedChangedEventHandler(CheckBox sender);
        public event CheckedChangedEventHandler CheckedChanged;

        public IList<Node> ItemSources { get; set; }

        private void CheckBox_Checked(object sender, System.Windows.RoutedEventArgs e)
        {
            CheckedChanged?.Invoke(sender as CheckBox);
        }
    }
    public class Node : INotifyPropertyChanged, ICloneable
    {
        public Node(Node Parent)
        {
            if (Parent == null)
                Generation = 1;
            else
                Generation = Parent.Generation + 1;
            this.Parent = Parent;
        }
        public void RegistPropertyChangedEvent()
        {
            PropertyChanged += (sender, e) => Parent.PropertyChanged?.Invoke(sender, e);
        }

        public string DisplayName { get; set; }

        public bool IsChecked { get; set; }
        public bool IsFile { get; set; }
        private Node _Parent;
        public Node Parent
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
        private List<Node> _Nodes = new List<Node>();
        public List<Node> Nodes
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
        public event PropertyChangedEventHandler PropertyChanged;


        public object Clone()
        {
            var temp = this.MemberwiseClone() as Node;
            temp.Nodes = Nodes.Select(x =>
            {
                Node node = x.Clone() as Node;
                node._Parent = temp;
                return node;
            }
            ).ToList();
            return temp;
        }
    }
    public class RelayCommand : ICommand
    {
        #region Fields 
        readonly Action<object> _execute;
        readonly Predicate<object> _canExecute;
        #endregion // Fields 
        #region Constructors 
        public RelayCommand(Action<object> execute) : this(execute, null) { }
        public RelayCommand(Action<object> execute, Predicate<object> canExecute)
        {
            _execute = execute ?? throw new ArgumentNullException("execute"); _canExecute = canExecute;
        }
        #endregion // Constructors 
        #region ICommand Members 
        [System.Diagnostics.DebuggerStepThrough]
        public bool CanExecute(object parameter)
        {
            return _canExecute == null ? true : _canExecute(parameter);
        }
        public event EventHandler CanExecuteChanged
        {
            add { CommandManager.RequerySuggested += value; }
            remove { CommandManager.RequerySuggested -= value; }
        }
        public void Execute(object parameter) { _execute(parameter); }
        #endregion // ICommand Members 
    }
}
